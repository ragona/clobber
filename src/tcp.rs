//! # TCP handling
//!
//! This file contains the TCP handling for `clobber`. The loop here is that we connect, write,
//! and then read. If the client is in repeat mode then it will repeatedly write/read while the
//! connection is open.
//!
//! ## Implementation notes
//!
//! `clobber`
//!
//! ## Performance Notes
//!
//! ### - Limit open ports and files
//!
//! Two of the key limiting factors for high TCP client throughput are running out of ports, or
//! opening more files than the underlying OS will allow. `clobber` tries to minimize issues here
//! by giving users control over the max connections. (It's also a good idea to check out your
//! specific `ulimit -n` settings and raise the max number of open files.)
//!
//! #### - Avoid cross-thread communication
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
//! futures timer it'd be a great contribution to the Rust community!*)
//!


use std::net::SocketAddr;
use std::time::{Duration, Instant};

use futures::executor::LocalPool;
use futures::io;
use futures::prelude::*;
use futures::task::SpawnExt;

use log::{debug, error, info, warn};
use romio::TcpStream;

use crate::Message;
use futures_timer::Delay;

/// Settings for the load test
///
/// todo: Make write/read optional. (Enum?)
///
#[derive(Debug, Copy, Clone)]
pub struct Config {
    /// Socket address (ip and port) of the host we'll be calling
    pub target: SocketAddr,
    /// Connections is the a key knob to turn when tuning a performance test. Honestly
    /// 'connections' isn't the best name; it implies a certain persistance that
    pub connections: u32,
    /// Optional rate-limiting. Precisely timing high rates is unreliable; if you're
    /// seeing slower than expected performance try running with no rate limit at all.
    pub rate: Option<u32>,
    /// Optional duration. If duration is None, clobber will run indefinitely.
    pub duration: Option<Duration>,
    /// Number of OS threads to distribute work between. 0 becomes num_cpus.
    pub threads: Option<u32>,
    /// Optionally time out requests at a number of milliseconds. Note: checking timeouts
    /// costs CPU cycles; max performance will suffer. However, if you have an ill-behaving
    /// server that isn't connecting consistently and is hanging onto connections, this can
    /// improve the situation.
    pub connect_timeout: Option<u32>,
    /// Optionally time out read requests at a number of milliseconds. Note: checking timeouts
    /// costs CPU cycles; max performance will suffer. However, if you have an ill-behaving server
    /// that isn't sending EOF bytes or otherwise isn't dropping connections, this can be
    /// essential to maintaing a high(ish) throughput, at the cost of more CPU load.
    pub read_timeout: Option<u32>,
    /// Absolute number of requests to be made. Should split evenly across threads.
    pub limit: Option<u32>,
}

impl Config {
    // todo: builder pattern
    pub fn new(target: SocketAddr, connections: u32) -> Config {
        Config {
            target,
            connections,
            rate: None,
            limit: None,
            duration: None,
            threads: None,
            read_timeout: None,
            connect_timeout: None,
        }
    }

    /// Number of user-defined threads, or all the threads on the host.
    pub fn num_threads(&self) -> u32 {
        match self.threads {
            None => num_cpus::get() as u32,
            Some(n) => n,
        }
    }

    /// Number of connection loops each thread should maintain. Will never be less than the number
    /// of threads.
    pub fn connections_per_thread(&self) -> u32 {
        match self.connections / self.num_threads() as u32 {
            0 => 1,
            n => n,
        }
    }

    /// The amount a single connection should wait between loops in order to maintain the defined
    /// rate. Returns a default loop if there is no rate.
    pub fn connection_delay(&self) -> Duration {
        match self.rate {
            Some(rate) => {
                Duration::from_secs(1) / rate * self.connections_per_thread() * self.num_threads()
            }
            None => Duration::default()
        }
    }

    /// The number of iterations each connection loop should perform before stopping. Doesn't play
    /// nice with limits that are not divisible by the number of threads and connections.
    pub fn limit_per_connection(&self) -> Option<u32> {
        match self.limit {
            None => None,
            Some(n) => Some(n / self.connections),
        }
    }
}

/// This function's goal is to make as many TCP requests as possible. Two common blockers
/// for achieving high TCP throughput are getting capped on number of open file descriptors,
/// or running out of available ports. `clobber` tries to minimize open ports and files by
/// limiting the number of active requests via the `connections` argument.
///
/// `clobber` will create `connections` number of async futures, distribute them across `threads`
/// threads (defaults to num_cpus), and each future will perform requests in a tight loop. If
/// there is a `rate` specified, there will be an optional delay to stay under the requested rate.
/// The futures are driven by a LocalPool executor, and there is no cross-thread synchronization
/// or communication with the default config. Note: for maximum performance avoid use of the
/// `rate`, `connect_timeout`, and `read_timeout` options.
///
pub fn clobber(config: Config, message: Message) -> std::io::Result<()> {
    info!("Starting: {:#?}", config);

    let mut threads = Vec::with_capacity(config.num_threads() as usize);

    for _ in 0..config.num_threads() {
        // per-thread clones
        let config = config.clone();
        let message = message.clone();

        // start thread which will contain a chunk of connections
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

        // spread out threads within a second
        std::thread::sleep(Duration::from_secs(1) / config.num_threads());
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
async fn connection(message: Message, config:Config) -> io::Result<()> {
    let start = Instant::now();

    let mut count = 0;
    let mut loop_complete = move || {
        count += 1;

        if let Some(duration) = config.duration {
            if Instant::now() >= start + duration {
                return true
            }
        }

        if let Some(limit) = config.limit_per_connection(){
            if count > limit {
                return true
            }
        }

        false
    };

    while !loop_complete() {
        let request_start = Instant::now();
        // A bit of a connundrum here is that these methods are reliant
        // on the underlying OS to time out. Your kernel will do that REALLY
        // SLOWLY, so if you're reading from a server that doesn't send an
        // EOF you're gonna have a bad time. However, explicitly timing out
        // futures is expensive to the point that I'm seeing nearly double the
        // throughput by not using futures-timer for timeouts.
        // todo: add optional timeouts for ill-behaving servers
        if let Ok(mut stream) = connect(&config.target).await {
            if let Ok(_) = write(&mut stream, &message.body).await {
                read(&mut stream).await.ok();
            }
        }

        if config.rate.is_some() {
            let elapsed = Instant::now() - request_start;
            if elapsed < config.connection_delay() {
                Delay::new(config.connection_delay() - elapsed).await.unwrap();
            } else {
                warn!("running behind; consider adding more connections");
            }
        }
    }

    Ok(())
}

/// Connects to the provided address, logs any errors and returns errors encountered.
async fn connect(addr: &SocketAddr) -> io::Result<TcpStream> {
    match TcpStream::connect(&addr).await {
        Ok(stream) => {
            debug!("connected to {}", &addr);
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

/// Writes provided buffer to the provided address, logs errors, returns errors encountered.
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

/// Reads from stream, logs any errors, returns errors encountered
async fn read(stream: &mut TcpStream) -> io::Result<usize> {
    let mut read_buffer = vec![]; // todo: size?
    match stream.read_to_end(&mut read_buffer).await {
        Ok(_) => {
            let n = read_buffer.len();
            debug!("{} bytes read ", n);
            Ok(n)
        }
        Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
            warn!("read timeout: {:?}", stream);
            Err(io::Error::new(io::ErrorKind::TimedOut, "foo"))
        }
        Err(e) => {
            error!("read error: '{}'", e);
            Err(e)
        }
    }

    // todo: Do something with the read_buffer?
    // todo: Perf testing on more verbose logging for analysis
}
