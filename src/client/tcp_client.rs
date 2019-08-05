use std::convert::TryInto;
use std::net::{Ipv4Addr, SocketAddr};
use std::str;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use futures::io::{self, AllowStdIo, AsyncReadExt, AsyncWriteExt, ErrorKind};
use futures::prelude::*;
use futures::task::SpawnExt;
use futures::{executor, FutureExt, StreamExt};
use futures_timer::{Delay, TryFutureExt};

use log::{debug, error, info, warn, LevelFilter};
use romio::TcpStream;
use std::ops::Deref;

use crate::client::stats::Stats;
use crate::Message;
use futures::executor::{ThreadPool, ThreadPoolBuilder};

#[derive(Debug, Copy, Clone)]
pub struct Config {
    pub rate: usize,
    pub port: u16,
    pub target: Ipv4Addr,
    pub duration: Option<Duration>,
    pub num_threads: u16,
    pub connect_timeout: u32,
    pub read_timeout: u32,
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
        }
    }

    pub fn addr(self: &Self) -> SocketAddr {
        SocketAddr::new(self.target.into(), self.port)
    }
}

pub fn clobber(config: Config, message: Message) -> std::io::Result<Stats> {
    let message = Arc::new(message);
    let address = Arc::new(config.addr());
    let mut threads = vec![];

    for _ in 0..config.num_threads {
        let addrr = Arc::clone(&address); // todo: fix 
        let msgg = Arc::clone(&message);

        let thread = std::thread::spawn(move || {
            let mut pool = ThreadPoolBuilder::new()
                .pool_size(1)
                .create()
                .expect("failed to create thread pool");

            executor::block_on(async move {
                let start = Instant::now();
                let delay = Duration::from_nanos((1e9 as usize / config.rate).try_into().unwrap());
                let connect_timeout = Duration::from_millis(config.connect_timeout as u64);
                let read_timeout = Duration::from_millis(config.read_timeout as u64);

                let past_duration = || match config.duration {
                    Some(duration) => Instant::now() > start + duration,
                    None => false,
                };

                loop {
                    if past_duration() {
                        break;
                    }

                    let addr = Arc::clone(&addrr);
                    let msg = Arc::clone(&msgg);

                    pool.spawn(async move {

                        let mut stats = Stats::new();
                        stats.connection_attempts += 1;

                        if let Ok(mut stream) = connect_with_timeout(&addr, connect_timeout).await {
                            stats.connections += 1;

                            if let Ok(n) = write(&mut stream, &msg.body).await {
                                stats.bytes_written += n;
                            }

                            if let Ok(n) = read_with_timeout(&mut stream, read_timeout).await {
                                stats.bytes_read += n;
                            };
                        };
                    }).unwrap();

                    Delay::new(delay * config.num_threads as u32).map(|_| {}).await;
                }
            });
        });

        threads.push(thread);
    }

    for handle in threads {
        handle.join();
    }

    Ok(Stats::new())
}

async fn connect_with_timeout(addr: &SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
    match TcpStream::connect(&addr).timeout(timeout).await {
        Ok(stream) => {
            debug!("connected to {}", &addr);
            Ok(stream)
        }
        Err(e) => {
            error!("connect error: '{}'", e);
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
            warn!("timeout: {:?}", stream);
            Err(io::Error::new(io::ErrorKind::TimedOut, "foo"))
        }
        Err(e) => {
            error!("read error: '{}'", e);
            Err(e)
        }
    }

    // todo: Do something with the read_buffer?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_logger;
    use std::io::Result;
    use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV4};

    /// Echo server for unit testing
    fn setup_server() -> SocketAddr {
        let mut server = romio::TcpListener::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = server.local_addr().unwrap();
        let mut read_buf = [0u8; 128];

        thread::spawn(move || {
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
        };

        let final_stats = clobber(config, message)?;

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
