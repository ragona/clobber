#![feature(async_await)]

use std::time::Duration;
use std::net::SocketAddr;

use futures::executor;
use futures::prelude::*;

use clobber::client::{self, Message, Stats, Config};
use crossbeam_channel::Receiver;

/// Echo server for testing
fn test_server() -> (SocketAddr, Receiver<Stats>) {
    let mut server = romio::TcpListener::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let mut read_buf = [0u8; 128];
    let addr = server.local_addr().unwrap();
    let (tx, rx) = crossbeam_channel::unbounded();

    std::thread::spawn(move || {
        executor::block_on(async move {
            let mut incoming = server.incoming();
            while let Some(stream) = incoming.next().await {
                match stream {
                    Ok(mut stream) => {
                        let tx = tx.clone();
                        let mut stats = Stats::new();
                        stats.connections += 1;
                        juliex::spawn(async move {
                            stream.read(&mut read_buf).await.unwrap();
                            stats.bytes_read += read_buf.len();
                            stream.write(&read_buf).await.unwrap();
                            stream.close().await.unwrap();

                            tx.send(stats).unwrap();
                        });
                    }
                    Err(e) => {
                        panic!(e);
                    }
                }
            }
        })
    });

    (addr, rx)
}

fn test_message() -> Message {
    Message::new(
        b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec()
    )
}

fn get_stats(receiver: Receiver<Stats>) -> Stats {
    let mut stats = Stats::new();
    while let Ok(result) = receiver.try_recv() {
        stats = stats + result;
    }

    stats
}

#[test]
fn slow() -> std::io::Result<()> {
    let (addr, receiver) = test_server();
    let config = Config {
        target: addr,
        rate: Some(10),
        duration: Some(Duration::from_secs(3)),
        num_threads: 0,
        connect_timeout: 100,
        read_timeout: 100,
        connections: 2,
    };

    client::tcp::clobber(config, test_message())?;

    let stats = get_stats(receiver);
    let wanted_duration = config.duration.unwrap().as_secs();
    let actual_duration = (stats.end_time - stats.start_time).as_secs();

    assert_eq!(actual_duration, wanted_duration);
    assert_eq!(config.rate.unwrap() * config.duration.unwrap().as_secs() as u32, stats.connections as u32);

    Ok(())
}
