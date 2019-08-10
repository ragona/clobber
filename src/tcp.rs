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

#[derive(Debug, Copy, Clone)]
pub struct Config {
    pub rate: Option<u32>,
    pub target: SocketAddr,
    pub duration: Option<Duration>,
    pub num_threads: u32,
    pub connect_timeout: u32,
    pub read_timeout: u32,
    pub connections: u32,
}

impl Config {
    pub fn new(target: SocketAddr) -> Config {
        Config {
            target,
            rate: None,
            duration: None,
            num_threads: 1,
            connect_timeout: 250,
            read_timeout: 250,
            connections: 10,
        }
    }
}

/// This function's goal is to make as many TCP requests as possible. Two common blockers
/// for achieving high TCP throughput are getting capped on number of open file descriptors,
/// or running out of available ports. It helps to avoid bursts of traffic, so this function
/// spreads out requests as much as possible across both thread and time.
///
/// If no `rate` is supplied, `clobber` will create `connections` number of async futures,
/// distribute them across `threads` threads (defaults to num_cpus), and each future will perform
/// requests in a tight loop. If there is a rate specified, there will be an optional sleep to stay
/// under the requested rate. The futures are driven by a LocalPool executor, and there is no
/// cross-thread synchronization or communication.
///
/// 4 threads, 8 connections:
/// --------------------------------------------------
/// thread 1:  a       e       a       e
/// thread 2:    b       f       b       f
/// thread 3:      c       g       c       g
/// thread 4:        d       h       d       h
/// --------------------------------------------------
///
pub fn clobber(config: Config, message: Message) -> std::io::Result<()> {
    info!("Starting: {:#?}", config);

    let num_threads = match config.num_threads {
        0 => num_cpus::get() as u32,
        n => n,
    };

    // things get weird if you have fewer connections than threads
    let conns_per_thread = match config.connections / num_threads as u32 {
        0 => 1,
        n => n,
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

                        // connect, write, read in a tight loop
                        loop {
                            if let Some(duration) = config.duration {
                                if Instant::now() >= start + duration {
                                    break;
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
                        }
                    })
                    .unwrap();
            }

            pool.run();
        });

        threads.push(thread);

        // stagger the start of each thread by a single tick
        std::thread::sleep(tick);
    }

    for handle in threads {
        handle.join().unwrap();
    }

    Ok(())
}

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
