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
use failure::_core::ops::Add;
use crossbeam_channel::{RecvError, TryRecvError, Receiver, Sender};

#[derive(Debug, Copy, Clone)]
pub struct Settings {
    pub rate: u64,
    pub port: u16,
    pub target: Ipv4Addr,
    pub duration: Option<Duration>,
    pub num_threads: u16,
    pub connect_timeout: u32,
}

impl Settings {
    pub fn new(target: Ipv4Addr, port: u16) -> Settings {
        Settings {
            port,
            target,
            rate: 1,
            duration: None,
            num_threads: 1,
            connect_timeout: 500,
        }
    }

    pub fn addr(self: &Self) -> SocketAddr {
        SocketAddr::new(self.target.into(), self.port)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Stats {
    bytes_read: u128,
    bytes_written: u128,
    connections: u64,
    connection_attempts: u64,
    duration: Duration,
    errors: u64,
}

impl Stats {
    pub fn new() -> Stats {
        Stats {
            bytes_read: 0,
            bytes_written: 0,
            connections: 0,
            connection_attempts: 0,
            duration: Default::default(),
            errors: 0,
        }
    }
}

impl Add for Stats {
    type Output = Stats;

    fn add(self: Stats, other: Stats) -> Stats {
        Stats {
            bytes_read: self.bytes_read + other.bytes_read,
            bytes_written: self.bytes_written + other.bytes_written,
            connections: self.connections + other.connections,
            connection_attempts: self.connection_attempts + other.connection_attempts,
            duration: self.duration,
            errors: self.errors + other.errors,
        }
    }
}

pub fn clobber(settings: Settings, close_receiver: Receiver<()>, result_sender: Sender<Stats>) -> std::io::Result<()> {
    let addr = settings.addr();
    let start = Instant::now();
    let delay = Duration::from_nanos(1e9 as u64 / settings.rate);
    let (mut stat_sender, mut stat_receiver) = crossbeam_channel::unbounded();


    thread::spawn(move || {
        let mut final_stats = Stats::new();

        while !close_requested(&close_receiver) {
            match stat_receiver.try_recv() {
                Ok(stat) => {
                    final_stats = final_stats + stat;
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
                Err(TryRecvError::Empty) => {}
            }
        }

        result_sender.send(final_stats);
    });

    executor::block_on(async move {

        loop {
            // todo catch ctrl c from channel, graceful stop

            match settings.duration {
                Some(duration) => {
                    if Instant::now() > start + duration {
                        break;
                    }
                }
                _ => {}
            }

            let s = stat_sender.clone();
            let mut result = Stats::new();

            juliex::spawn(async move {
                result.connection_attempts += 1;

                match TcpStream::connect(&addr)
                    .timeout(Duration::from_millis(settings.connect_timeout as u64))
                    .await
                    {
                        Ok(stream) => {
                            result.connections += 1;
                            // read, write
                            info!("{:?}", stream);
                        }
                        Err(ref e) if e.kind() == ErrorKind::TimedOut => {
                            // continue
                            info!("timeout");
                        }
                        Err(e) => {
                            result.errors += 1;
                            // continue
                            error!("{}", e);
                        }
                    };

                s.send(result);
            });

            spin_sleep::sleep(delay);
        }
    });

    Ok(())
}

fn close_requested(close: &Receiver<()>) -> bool {
    match close.try_recv() {
        Ok(_) | Err(TryRecvError::Disconnected) => true,
        Err(TryRecvError::Empty) => false,
    }
}

fn listen_for_shutdown() {

}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_logger;
    use std::io::Result;
    use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV4};

    fn test_settings(addr: &SocketAddr) -> Settings {
        let target = match addr.ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => unimplemented!(),
        };

        Settings {
            rate: 10,
            port: addr.port(),
            target,
            duration: Some(Duration::from_millis(1000)),
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
    fn slow_clobber() -> std::io::Result<()> {
        setup_logger(LevelFilter::Info);
        let addr = setup_server();
        let (close_sender, close_receiver ) = crossbeam_channel::unbounded();
        let (stat_sender, stat_receiver) = crossbeam_channel::unbounded();
        let mut settings = test_settings(&addr);

        clobber(settings, close_receiver, stat_sender)?;
        dbg!("clobber done");
        let final_stats = stat_receiver.recv().unwrap();

        dbg!(final_stats);

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
