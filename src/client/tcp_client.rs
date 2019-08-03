use std::net::{Ipv4Addr, SocketAddr};
use std::str;
use std::thread;
use std::time::{Duration, Instant};

use futures::io::{self, AllowStdIo, AsyncReadExt, AsyncWriteExt, ErrorKind};
use futures::prelude::*;
use futures::task::SpawnExt;
use futures::{executor, FutureExt, StreamExt};
use futures_timer::{Delay, TryFutureExt};

use juliex;
use log::{error, info, LevelFilter};
use romio::TcpStream;

#[derive(Debug, Copy, Clone)]
pub struct ClientSettings {
    pub rate: u64,
    pub port: u16,
    pub target: Ipv4Addr,
    pub duration: Option<Duration>,
    pub connections: u16,
    pub num_threads: u16,
    pub connect_timeout: u32,
}

impl ClientSettings {
    pub fn new(target: Ipv4Addr, port: u16) -> ClientSettings {
        ClientSettings {
            port,
            target,
            rate: 1,
            duration: None,
            connections: 1,
            num_threads: 1,
            connect_timeout: 500,
        }
    }

    pub fn addr(self: &Self) -> SocketAddr {
        SocketAddr::new(self.target.into(), self.port)
    }
}

pub fn clobber(settings: ClientSettings) -> std::io::Result<()> {
    let addr = settings.addr();
    let start = Instant::now();
    let delay = Duration::from_nanos(1e9 as u64 / settings.rate);

    executor::block_on(async move {
        loop {
            // todo catch ctrl c from channel

            match settings.duration {
                Some(duration) => {
                    if Instant::now() > start + duration {
                        break;
                    }
                }
                _ => {}
            }

            juliex::spawn(async move {
                match TcpStream::connect(&addr)
                    .timeout(Duration::from_millis(settings.connect_timeout as u64))
                    .await
                {
                    Ok(stream) => {
                        info!("{:?}", stream);
                    }
                    Err(ref e) if e.kind() == ErrorKind::TimedOut => {
                        info!("timeout");
                    }
                    Err(e) => {
                        error!("{}", e);
                    }
                };
            });

            spin_sleep::sleep(delay);
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_logger;
    use std::io::Result;
    use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV4};

    fn test_settings(addr: &SocketAddr) -> ClientSettings {
        let target = match addr.ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => unimplemented!(),
        };

        ClientSettings {
            rate: 10,
            port: addr.port(),
            target,
            duration: Some(Duration::from_millis(500)),
            connections: 1,
            num_threads: 1,
            connect_timeout: 200,
        }
    }

    /// Echo server for unit testing
    fn setup_server() -> SocketAddr {
        let mut server = romio::TcpListener::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = server.local_addr().unwrap();

        thread::spawn(move || {
            executor::block_on(async {
                let mut incoming = server.incoming();
                while let Some(stream) = incoming.next().await {
                    match stream {
                        Ok(mut stream) => {
                            juliex::spawn(async move {
                                let (mut reader, mut writer) = stream.split();
                                reader.copy_into(&mut writer).await.unwrap();
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
    fn single_thread_clobber() -> std::io::Result<()> {
        let addr = setup_server();

        clobber(test_settings(&addr))?;

        // todo: add some assertions

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
            TcpStream::connect(&addr)
                .timeout(Duration::from_nanos(1))
                .await
        });

        let timed_out = match result {
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => true,
            _ => false,
        };

        assert!(timed_out);

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

            loop {
                match stream
                    .read_to_end(&mut read_buffer)
                    .timeout(Duration::from_millis(100))
                    .await
                {
                    Ok(n) => {}
                    Err(_) => {
                        break;
                    }
                };
            }

            Ok::<_, io::Error>(())
        })?;

        assert_eq!(&write_buffer, &read_buffer.as_slice());

        Ok(())
    }
}
