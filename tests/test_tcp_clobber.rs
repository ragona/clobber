#![feature(async_await)]

use std::net::SocketAddr;

use futures::executor;
use futures::prelude::*;

use clobber::{tcp, Message, Stats, ConfigBuilder};
use crossbeam_channel::Receiver;

/// Echo server for testing
/// todo: Allow tests to pass in an enum to configure how the server behaves. (e.g. Echo vs. static.)
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
                            stats.bytes_written += read_buf.len();
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
    Message::new(b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec())
}

fn get_stats(receiver: Receiver<Stats>) -> Stats {
    let mut stats = Stats::new();
    while let Ok(result) = receiver.try_recv() {
        stats = stats + result;
    }

    stats
}

#[test]
fn single_thread_limited_rate_and_total() -> std::io::Result<()> {
    let (addr, receiver) = test_server();

    let config = ConfigBuilder::new(addr)
        .connections(10)
        .rate(Some(10))
        .limit(Some(20))
        .threads(Some(1))
        .build();

    tcp::clobber(config, test_message())?;

    let stats = get_stats(receiver);
    let total = config.limit.unwrap();

    assert_eq!(total, stats.connections as u32);

    Ok(())
}

#[test]
fn multi_thread_limited_rate_and_total() -> std::io::Result<()> {
    let (addr, receiver) = test_server();

    let config = ConfigBuilder::new(addr)
        .rate(Some(10))
        .limit(Some(20))
        .connections(10)
        .threads(Some(2))
        .build();

    tcp::clobber(config, test_message())?;

    let stats = get_stats(receiver);
    let total = config.limit.unwrap();

    assert_eq!(total, stats.connections as u32);

    Ok(())
}