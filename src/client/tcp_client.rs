use std::net::{Ipv4Addr, SocketAddr};
use std::thread;
use std::time::Duration;

use futures::executor;
use futures::io::{self, AllowStdIo, AsyncReadExt, AsyncWriteExt, ErrorKind};
use futures_timer::TryFutureExt;
use juliex;
use log::{info, LevelFilter};
use romio::TcpStream;

#[derive(Debug, Copy, Clone)]
pub struct ClientSettings {
    pub connections: u16,
    pub num_threads: u16,
    pub target: Ipv4Addr,
    pub port: u16,
    pub rate: u64,
    // todo: Add duration of run
}

impl ClientSettings {
    pub fn new(target: Ipv4Addr, port: u16) -> ClientSettings {
        ClientSettings {
            rate: 1,
            connections: 1,
            num_threads: 1,
            target,
            port,
        }
    }

    pub fn addr(self: &Self) -> SocketAddr {
        SocketAddr::new(self.target.into(), self.port)
    }
}

pub fn clobber(settings: ClientSettings) -> std::io::Result<()> {
    let delay = 1e9 as u64 / settings.rate;
    let connections: Vec<TcpStream> = vec![];

    //    executor::block_on(async {
    //        for _ in 0..settings.connections {
    //            let mut stream = connect(&settings).await.unwrap();
    //
    //            loop {
    //                juliex::spawn(async move {
    //                    &stream.write_all(&REQUEST).await.unwrap();
    //
    //                    Ok(());
    //                });
    //            }
    //        }
    //    });

    Ok(())
}

/// Todo list:
/// - Figure ownership issues to forcibly drop the test server so the socket closes.
/// - Read
#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_logger;
    use futures::StreamExt;
    use std::io::Result;
    use std::net::{SocketAddr, SocketAddrV4};

    fn setup_test_server() -> SocketAddr {
        let addr = "127.0.0.1:8000".parse().unwrap();
        let response = b"ok";
        let mut server = romio::TcpListener::bind(&addr).unwrap();

        thread::spawn(move || {
            let mut incoming = server.incoming();

            executor::block_on(async {
                match incoming.next().await {
                    Some(stream) => {
                        match stream {
                            Ok(mut stream) => {
                                stream.write_all(response).await.unwrap();
                            }
                            Err(_) => { /* connection failed */ }
                        }
                    }
                    None => {}
                }
            })
        });

        addr
    }

    #[test]
    fn test_clobber() -> std::io::Result<()> {
        //        let addr = setup_test_server();

        //        clobber(settings)?;

        Ok(())
    }

    #[test]
    fn test_connect() -> Result<()> {
        let addr = setup_test_server();
        executor::block_on(async {
            TcpStream::connect(&addr)
                .timeout(Duration::from_millis(100))
                .await?;

            Ok::<_, io::Error>(())
        })?;

        Ok(())
    }
    #[test]
    fn test_connect_timeout() -> std::io::Result<()> {
        let addr = setup_test_server();
        let result = executor::block_on(async {
            TcpStream::connect(&addr)
                .timeout(Duration::from_millis(100))
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
    fn test_connect_then_write() -> std::io::Result<()> {
        let addr = setup_test_server();
        let buffer = b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n";

        executor::block_on(async {
            let mut stream = TcpStream::connect(&addr)
                .timeout(Duration::from_millis(100))
                .await?;

            stream.write_all(buffer).await?;

            Ok::<_, io::Error>(())
        })?;

        Ok(())
    }
}
