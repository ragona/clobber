use std::net::{Ipv4Addr, SocketAddr};
use std::str;
use std::thread;
use std::time::{Duration, Instant};

use futures::io::{self, AllowStdIo, AsyncReadExt, AsyncWriteExt, ErrorKind};
use futures::prelude::*;
use futures::task::SpawnExt;
use futures::{executor, FutureExt, StreamExt};
use futures_timer::{Delay, TryFutureExt};

use crossbeam_channel::{Receiver, RecvError, Sender, TryRecvError};
use failure::_core::ops::Add;
use juliex;
use log::{error, info, LevelFilter};
use romio::TcpStream;
use std::ops::Deref;

use crate::Message;
use std::sync::Arc;

#[derive(Debug, Copy, Clone)]
pub struct Settings {
    pub rate: u64,
    pub port: u16,
    pub target: Ipv4Addr,
    pub duration: Option<Duration>,
    pub num_threads: u16,
    pub connect_timeout: u32,
    pub read_timeout: u32,
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
            read_timeout: 500,
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
    start_time: Instant,
    end_time: Instant,
}

impl Stats {
    pub fn duration(self: &Self) -> Duration {
        self.end_time - self.start_time
    }
}

impl Stats {
    pub fn new() -> Stats {
        Stats {
            bytes_read: 0,
            bytes_written: 0,
            connections: 0,
            connection_attempts: 0,
            start_time: Instant::now(),
            end_time: Instant::now(),
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
            start_time: self.start_time,
            end_time: other.end_time, // todo: Is this right?
        }
    }
}

// todo: oof this method got big
pub fn clobber(
    settings: Settings,
    message: Message,
    close_receiver: Receiver<()>,
    result_sender: Sender<Stats>,
) -> std::io::Result<()> {
    let mut final_stats = Stats::new();
    let start = Instant::now();
    let delay = Duration::from_nanos(1e9 as u64 / settings.rate);
    let (stat_sender, stat_receiver) = crossbeam_channel::unbounded();
    let address = Arc::new(settings.addr());
    let message = Arc::new(message);

    thread::spawn(move || {
        loop {
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

        result_sender.send(final_stats).unwrap();
    });

    executor::block_on(async move {
        loop {
            match settings.duration {
                Some(duration) => {
                    if Instant::now() > start + duration {
                        break;
                    }
                }
                _ => {}
            }

            match close_receiver.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    break;
                }
                _ => {}
            }

            // perform necessary memory copies here
            // todo: scope down, revisit use of Arc
            let sender = stat_sender.clone();
            let addr = Arc::clone(&address);
            let msg = Arc::clone(&message);

            juliex::spawn(async move {
                let mut stats = Stats::new();
                stats.connection_attempts += 1;
                match TcpStream::connect(&addr)
                    .timeout(Duration::from_millis(settings.connect_timeout as u64))
                    .await
                {
                    Ok(mut stream) => {
                        stats.connections += 1;

                        match stream.write_all(&msg.body).await {
                            Ok(_) => {},
                            Err(e) => {
                                error!("{}", e);
                            }
                        }

                        stats.bytes_written += Arc::deref(&msg).body.len() as u128;

                        let mut read_buffer = vec![]; // todo: size?
                        match stream
                            .read_to_end(&mut read_buffer)
                            .timeout(Duration::from_millis(100))
                            .await
                        {
                            Ok(_) => {
                                stats.bytes_read += read_buffer.len() as u128;
                            }
                            Err(_) => {}
                        };
                    }
                    Err(ref e) if e.kind() == ErrorKind::TimedOut => {
                        // continue
                        info!("connection timeout");
                    }
                    Err(e) => {
                        // continue
                        error!("{}", e);
                    }
                };

                sender.send(stats).unwrap();
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

    /// Echo server for unit testing
    fn setup_server() -> SocketAddr {
        let mut server = romio::TcpListener::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = server.local_addr().unwrap();

        thread::spawn(move || {
            executor::block_on(async {
                let mut incoming = server.incoming();
                while let Some(stream) = incoming.next().await {
                    match stream {
                        Ok(stream) => {
                            juliex::spawn(async move {
                                let (reader, mut writer) = stream.split();
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
        setup_logger(LevelFilter::Info).unwrap();

        let addr = setup_server();
        let buffer = b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec();
        let message = Message::new(buffer);
        let (_close_sender, close_receiver) = crossbeam_channel::unbounded();
        let (result_sender, result_receiver) = crossbeam_channel::unbounded();

        let target = match addr.ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => unimplemented!("todo: support ipv6"),
        };

        let settings = Settings {
            rate: 10,
            port: addr.port(),
            target,
            duration: Some(Duration::from_millis(1000)),
            num_threads: 1,
            connect_timeout: 200,
            read_timeout: 5000,
        };

        clobber(settings, message, close_receiver, result_sender)?;

        let final_stats = result_receiver.recv().unwrap();

        assert_eq!(final_stats.connection_attempts, final_stats.connections);
        assert_eq!(settings.rate, final_stats.connections);

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

        assert_eq!(&write_buffer, &read_buffer.as_slice());

        Ok(())
    }
}
