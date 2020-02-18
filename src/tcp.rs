//! # TCP handling
//!
//! This file contains the TCP handling for `clobber`. The loop here is that we connect, write,
//! and then read. If the client is in repeat mode then it will repeatedly write/read while the
//! connection is open.
//!
//! ## Performance Notes
//!
//! ### Perform allocations at startup
//!
//! The pool of connections is created up front, and then connections begin sending requests
//! to match the defined rate. (Or in the case of no defined, they start immediately.) In general
//! we try to to limit significant allocations to startup rather than doing them on the fly.
//! More specifically, you shouldn't see any of these behaviors inside the tight `while` loop
//! inside the `connection()` method.
//!
//! ### Limit open ports and files
//!
//! Two of the key limiting factors for high TCP client throughput are running out of ports, or
//! opening more files than the underlying OS will allow. `clobber` tries to minimize issues here
//! by giving users control over the max connections. (It's also a good idea to check out your
//! specific `ulimit -n` settings and raise the max number of open files.)
//!
//! #### Avoid cross-thread communication
//! This library uses no cross-thread communication via `std::sync` or `crossbeam`. All futures
//! are executed on a `LocalPool`, and the number of OS threads used is user configurable. This
//! has a number of design impacts. For example, it becomes more difficult to aggregate what each
//! connection is doing. This is simple if you just pass the results to a channel, but this has a
//! non-trivial impact on performance.
//!
//! *Note: This is currently violated by the way we accomplish rate limiting, which relies on a
//! global thread that manages timers. This ends up putting disproportionate load on that thread at
//! some point. But if you're relying on rate limiting you're trying to slow it down, so we're
//! putting this in the 'feature' column. (If anyone would like to contribute a thread-local
//! futures timer it'd be a great contribution to the Rust community!)
//!

use std::net::SocketAddr;
use std::time::Instant;

use async_std::io::{self};
use async_std::net::TcpStream;
use async_std::prelude::*;

// I'd like to remove this dependency, but async-std doesn't currently have a LocalPool executor
// todo: Revisit
use futures::executor::LocalPool;
use futures::task::SpawnExt;

use futures_timer::Delay;
use log::{debug, error, info, warn};

use crate::Config;

/// The overall test runner
///
/// This method contains the main core loop.
///
/// `clobber` will create `connections` number of async futures, distribute them across `threads`
/// threads (defaults to num_cpus), and each future will perform requests in a tight loop. If
/// there is a `rate` specified, there will be an optional delay to stay under the requested rate.
/// The futures are driven by a LocalPool executor, and there is no cross-thread synchronization
/// or communication with the default config. Note: for maximum performance avoid use of the
/// `rate`, `connect_timeout`, and `read_timeout` options.
///
pub fn clobber(config: Config, message: Vec<u8>) -> std::io::Result<()> {
    info!("Starting: {:#?}", config);
    let mut threads = Vec::with_capacity(config.num_threads() as usize);

    // configure fuzzing if a file has been provided in the config

    for _ in 0..config.num_threads() {
        // per-thread clones
        let message = message.clone();
        let config = config.clone();

        // start OS thread which will contain a chunk of connections
        let thread = std::thread::spawn(move || {
            let mut pool = LocalPool::new();
            let mut spawner = pool.spawner();

            // all connection futures are spawned up front
            for i in 0..config.connections_per_thread() {
                // per-connection clones
                let message = message.clone();
                let config = config.clone();

                spawner
                    .spawn(async move {
                        if config.rate.is_some() {
                            Delay::new(i * config.connection_delay());
                        }

                        connection(message, config)
                            .await
                            .expect("Failed to run connection");
                    })
                    .unwrap();
            }
            pool.run();
        });
        threads.push(thread);
    }
    for handle in threads {
        handle.join().unwrap();
    }

    Ok(())
}

/// Handles a single connection
///
/// This method infinitely loops, performing a connect/write/read transaction against the
/// configured target. If `repeat` is true in `config`, the loop will keep the connection alive.
/// Otherwise, it will drop the connection after successfully completing a read, and then it will
/// start over and reconnect. If it does not successfully read, it will block until the underlying
/// TCP read fails unless `read-timeout` is configured.
///
/// This is a long-running function that will continue making calls until it hits a time or total
/// loop count limit.
///
/// todo: This ignores both read-timeout and repeat
async fn connection(mut message: ByteMutator, config: Config) -> io::Result<()> {
    let start = Instant::now();

    let mut count = 0;
    let mut loop_complete = |config: &Config| {
        count += 1;

        if let Some(duration) = config.duration {
            if Instant::now() >= start + duration {
                return true;
            }
        }

        if let Some(limit) = config.limit_per_connection() {
            if count > limit {
                return true;
            }
        }

        false
    };

    let should_delay = |elapsed, config: &Config| match config.rate {
        Some(_) => {
            if elapsed < config.connection_delay() {
                true
            } else {
                warn!("running behind; consider adding more connections");
                false
            }
        }
        None => false,
    };

    // This is the guts of the application; the tight loop that executes requests
    let mut read_buffer = [0u8; 1024]; // todo variable size? :(
    while !loop_complete(&config) {
        // todo: add optional timeouts back
        let request_start = Instant::now();
        if let Ok(mut stream) = connect(&config.target).await {
            // one write/read transaction per repeat
            for _ in 0..config.repeat {
                if write(&mut stream, message.read()).await.is_ok() {
                    read(&mut stream, &mut read_buffer).await.ok();
                }
            }

            // todo: analysis

            // advance mutator state (no-op with no fuzzer config)
            message.next();
        }

        if config.rate.is_some() {
            let elapsed = Instant::now() - request_start;
            if should_delay(elapsed, &config) {
                Delay::new(config.connection_delay() - elapsed)
                    .await
                    .unwrap();
            }
        }
    }

    Ok(())
}

/// Connects to the provided address, logs, returns Result<TcpStream, io::Error>
async fn connect(addr: &SocketAddr) -> io::Result<TcpStream> {
    match TcpStream::connect(addr).await {
        Ok(stream) => {
            debug!("connected to {}", addr);
            Ok(stream)
        }
        Err(e) => {
            if e.kind() != io::ErrorKind::TimedOut {
                error!("unknown connect error: '{}'", e);
            }
            Err(e)
        }
    }
}

/// Writes provided buffer to the provided address, logs, returns Result<bytes_written, io::Error>
async fn write(stream: &mut TcpStream, buf: &[u8]) -> io::Result<usize> {
    match stream.write_all(buf).await {
        Ok(_) => {
            let n = buf.len();
            debug!("{} bytes written", n);
            Ok(n)
        }
        Err(e) => {
            error!("write error: '{}'", e);
            Err(e)
        }
    }
}

/// Reads from stream, logs, returns Result<num_bytes_read, io::Error>
async fn read(stream: &mut TcpStream, mut read_buffer: &mut [u8]) -> io::Result<usize> {
    match stream.read(&mut read_buffer).await {
        Ok(n) => {
            debug!("{} bytes read ", n);
            Ok(n)
        }
        Err(e) => {
            error!("read error: '{}'", e);
            Err(e)
        }
    }

    // todo: Do something with the read_buffer?
    // todo: More verbose logging; dump to stdout, do post-run analysis on demand
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::echo_server;

    #[test]
    fn test_connect() {
        let result = async_std::task::block_on(async {
            let addr = echo_server().unwrap();
            let result = connect(&addr).await;

            result
        });

        assert!(result.is_ok());
    }

    #[test]
    fn test_write() {
        let addr = echo_server().unwrap();
        let input = "test".as_bytes();
        let want = input.len();

        let result = async_std::task::block_on(async move {
            let mut stream = connect(&addr).await?;
            let bytes_written = write(&mut stream, &input).await?;
            Ok::<_, io::Error>(bytes_written)
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), want);
    }

    #[test]
    fn test_read() {
        let addr = echo_server().unwrap();
        let input = "test\n\r\n".as_bytes();
        let want = input.len();

        let result = async_std::task::block_on(async move {
            let mut stream = connect(&addr).await?;
            let mut read_buffer = [0u8; 1024];
            let _ = write(&mut stream, &input).await?;
            let bytes_read = read(&mut stream, &mut read_buffer).await?;

            Ok::<_, io::Error>(bytes_read)
        });

        assert!(result.is_ok());
        assert_eq!(want, result.unwrap());
    }
}
