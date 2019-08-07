use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use futures::executor::LocalPool;
use futures::io::{self, AsyncReadExt};
use futures::prelude::*;
use futures::task::LocalSpawnExt;
use futures_timer::{Delay, TryFutureExt};

use log::{debug, error, info, warn};
use romio::TcpStream;

use crate::client::stats::Stats;
use crate::Message;

#[derive(Debug, Copy, Clone)]
pub struct Config {
    pub rate: usize,
    pub port: u16,
    pub target: Ipv4Addr,
    pub duration: Option<Duration>,
    pub num_threads: u16,
    pub connect_timeout: u32,
    pub read_timeout: u32,
    pub connections: u32,
}

impl Config {
    pub fn new(target: Ipv4Addr, port: u16) -> Config {
        Config {
            port,
            target,
            rate: 1,
            duration: None,
            num_threads: 1,
            connect_timeout: 500,
            read_timeout: 500,
            connections: 10,
        }
    }

    pub fn addr(self: &Self) -> SocketAddr {
        SocketAddr::new(self.target.into(), self.port)
    }
}

/// The goal of this function is to match the requested request rate as closely as
/// possible. Requests consist of a single transaction: connect, write, read.
/// disconnect, sleep. Requests are distributed across two axis; threads, and
/// connections. Connections are evenly distributed across threads. Each thread
/// has a LocalPool executor that asynchronously works on the thread's connections.
/// Connections are also evenly distributed in time so that the first connection
/// will not start again until after the last connection has started:
///
/// --------------------------------------------------
/// thread 1:  a       e       a       e
/// thread 2:    b       f       b       f
/// thread 3:      c       g       c       g
/// thread 4:        d       h       d       h
/// --------------------------------------------------
/// 
/// todo: The word 'connection' implies a persistence that doesn't exist. Fix?
///
pub fn clobber(config: Config, message: Message) -> std::io::Result<Stats> {

    info!("Starting: {:?}", config);

    // todo: add channels back at very end of run
    // todo: add back graceful ctrl+c handling (atomicbool?)
    // time variables
    let start = Instant::now();
    let tick_delay = tick_delay(&config);
    let read_timeout = Duration::from_millis(config.read_timeout as u64);
    let connect_timeout = Duration::from_millis(config.connect_timeout as u64);
    let conns_per_thread = config.connections / config.num_threads as u32;

    let mut threads = vec![];

    for _ in 0..config.num_threads {
        // per-thread clones
        let config = config.clone();
        let message = message.clone();
        let addr = config.addr();

        // start thread which will contain a chunk of connections
        let thread = std::thread::spawn(move || {
            let mut pool = LocalPool::new();
            let mut spawner = pool.spawner();

            // immediately create all connections, but don't let them start until it's their turn
            for i in 0..conns_per_thread {
                // per-connection clones
                let message = message.clone();

                spawner
                    .spawn_local(async move {
                        // stagger the start of each connection loop to spread out requests
                        Delay::new(tick_delay * config.num_threads as u32 * i)
                            .map(|_| {})
                            .await;

                        // connect, write, read loop
                        loop {
                            if let Ok(mut stream) =
                                connect_with_timeout(&addr, connect_timeout).await
                            {
                                write(&mut stream, &message.body).await.ok();
                                read_with_timeout(&mut stream, read_timeout).await.ok();
                            };

                            // wait before looping
                            // todo: account for how long the request took
                            Delay::new(tick_delay * config.num_threads as u32 * conns_per_thread)
                                .map(|_| {})
                                .await;

                            // bail out of the loop if we're past the duration
                            if let Some(duration) = config.duration {
                                if Instant::now() > start + duration {
                                    break;
                                }
                            }

                            // todo: atomicbool to stop thread
                        }
                    })
                    .unwrap();
            }

            pool.run();
        });

        threads.push(thread);

        // stagger the start of each thread
        std::thread::sleep(tick_delay);
    }

    for handle in threads {
        handle.join().unwrap();
    }

    // todo: return real stats
    Ok(Stats::new())
}

async fn connect_with_timeout(addr: &SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
    match TcpStream::connect(&addr).timeout(timeout).await {
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

async fn read_with_timeout(stream: &mut TcpStream, timeout: Duration) -> io::Result<usize> {
    let mut read_buffer = vec![]; // todo: size?
    match stream.read_to_end(&mut read_buffer).timeout(timeout).await {
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
}

/// Target duration between each individual request.
fn tick_delay(config: &Config) -> Duration {
    let second = 1e9 as u64; // 1B nanoseconds is a second
    let delay_in_nanos = second / config.rate as u64;

    Duration::from_nanos(delay_in_nanos as u64)
}

// todo: Move to tests folder
// todo: Add test server side results channel to verify client behavior

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::setup_logger;
    use futures::executor;
    use std::io::Result;
    use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV4};

    /// Echo server for unit testing
    fn setup_server() -> SocketAddr {
        let mut server = romio::TcpListener::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = server.local_addr().unwrap();
        let mut read_buf = [0u8; 128];

        std::thread::spawn(move || {
            executor::block_on(async {
                let mut incoming = server.incoming();
                while let Some(stream) = incoming.next().await {
                    match stream {
                        Ok(mut stream) => {
                            juliex::spawn(async move {
                                stream.read(&mut read_buf).await.unwrap();
                                stream.write(&read_buf).await.unwrap();
                                stream.close().await.unwrap();
                            });
                        }
                        Err(e) => {
                            panic!(e);
                        }
                    }
                }
            })
        });

        addr
    }

    #[test]
    fn slow_clobber() -> std::io::Result<()> {
        // setup_logger(LevelFilter::Debug).unwrap();

        //        let addr = setup_server();
        let addr: SocketAddr = "127.0.0.1:7878".parse().unwrap();
        let buffer = b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec();
        let message = Message::new(buffer);

        let target = match addr.ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => unimplemented!("todo: support ipv6"),
        };

        let config = Config {
            target,
            rate: 1000,
            port: addr.port(),
            duration: Some(Duration::from_millis(1000)),
            num_threads: 4,
            connect_timeout: 100,
            read_timeout: 100,
            connections: 200,
        };

        let final_stats = clobber(config, message)?;

        // todo: restore stats

        //        assert_eq!(final_stats.connection_attempts, final_stats.connections);
        //        assert_eq!(config.rate, final_stats.connections);

        dbg!(final_stats);

        Ok(())
    }

    #[test]
    fn connect() -> Result<()> {
        let addr = setup_server();
        executor::block_on(async {
            TcpStream::connect(&addr)
                .timeout(Duration::from_millis(100))
                .await?;

            Ok::<_, io::Error>(())
        })?;

        Ok(())
    }

    #[test]
    fn connect_timeout() -> std::io::Result<()> {
        let addr = setup_server();
        let result = executor::block_on(async {
            connect_with_timeout(&addr, Duration::from_nanos(1)).await
        });

        let _timed_out = match result {
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => true,
            _ => false,
        };

        // todo: this test only usually works -- disabling for now
        // assert!(timed_out);

        Ok(())
    }

    #[test]
    fn write() -> std::io::Result<()> {
        let addr = setup_server();
        let buffer = b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n";

        executor::block_on(async {
            let mut stream = TcpStream::connect(&addr)
                .timeout(Duration::from_millis(100))
                .await?;

            stream.write_all(buffer).await
        })?;

        Ok(())
    }

    #[test]
    fn read() -> std::io::Result<()> {
        let addr = setup_server();
        let write_buffer = b"GET / HTTP/1.1\r\n\r\n";
        let mut read_buffer = vec![];

        executor::block_on(async {
            let mut stream = TcpStream::connect(&addr)
                .timeout(Duration::from_millis(100))
                .await?;

            stream.write_all(write_buffer).await.unwrap();

            match stream
                .read_to_end(&mut read_buffer)
                .timeout(Duration::from_millis(100))
                .await
            {
                Ok(_) => {}
                Err(_) => {}
            };

            Ok::<_, io::Error>(())
        })?;

        assert_eq!(
            &write_buffer[..],
            &read_buffer.as_slice()[0..write_buffer.len()]
        );

        Ok(())
    }
}
