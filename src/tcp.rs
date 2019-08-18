//! # TCP business logic
//!
//! This file contains the bulk of the business logic for `clobber`.
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
    pub num_threads: Option<u32>,
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
            num_threads: None,
            read_timeout: None,
            connect_timeout: None,
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

    let num_threads = match config.num_threads {
        None => num_cpus::get() as u32,
        Some(n) => n,
    };

    // things get weird if you have fewer connections than threads
    let conns_per_thread = match config.connections / num_threads as u32 {
        0 => 1,
        n => n,
    };

    let limit_per_conn = match config.limit {
        None => None,
        Some(n) => Some(n / config.connections),
    };

    let start = Instant::now();
    let tick = match config.rate {
        Some(rate) => Duration::from_nanos(1e9 as u64 / rate as u64),
        None => Duration::default(),
    };

    let mut threads = Vec::with_capacity(num_threads as usize);

    for _ in 0..num_threads {
        // per-thread clones
        let addr = config.target.clone();
        let config = config.clone();
        let message = message.clone();

        // start thread which will contain a chunk of connections
        let thread = std::thread::spawn(move || {
            let mut pool = LocalPool::new();
            let mut spawner = pool.spawner();

            // all connection futures are spawned up front
            for i in 0..conns_per_thread {
                // per-connection clones
                let message = message.clone();
                let config = config.clone();

                spawner
                    .spawn(async move {
                        // spread out loop start times within a thread to smoothly match rate
                        if config.rate.is_some() {
                            Delay::new(tick * num_threads * i).await.unwrap();
                        }

                        // connect, write, read
                        let mut count = 0;
                        loop {
                            if let Some(duration) = config.duration {
                                if Instant::now() >= start + duration {
                                    break
                                }
                            }

                            if let Some(limit) = limit_per_conn {
                                if count >= limit {
                                    break
                                }
                            }

                            // A bit of a connundrum here is that these methods are reliant
                            // on the underlying OS to time out. Your kernel will do that REALLY
                            // SLOWLY, so if you're reading from a server that doesn't send an
                            // EOF you're gonna have a bad time. However, explicitly timing out
                            // futures is expensive to the point that I'm seeing nearly double the
                            // throughput by not using futures-timer for timeouts.
                            // todo: add optional timeouts for ill-behaving servers
                            let request_start = Instant::now();
                            if let Ok(mut stream) = connect(&addr).await {
                                if let Ok(_) = write(&mut stream, &message.body).await {
                                    read(&mut stream).await.ok();
                                }
                            }

                            if config.rate.is_some() {
                                let elapsed = Instant::now() - request_start;
                                let delay = tick * conns_per_thread * num_threads;
                                if elapsed < delay {
                                    Delay::new(delay - elapsed).await.unwrap();
                                } else {
                                    warn!("running behind; consider adding more connections");
                                }
                            }

                            count += 1;
                        }
                    })
                    .unwrap();
            }
            pool.run();
        });
        threads.push(thread);
        std::thread::sleep(tick / 2); // stagger the start of each thread by a single tick
    }
    for handle in threads {
        handle.join().unwrap();
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
