use std::net::SocketAddr;
use std::time::Duration;

use futures::executor;
use futures::prelude::*;

use crate::{tcp, Config, Message, Stats};
use crossbeam_channel::Receiver;

/// Echo server for testing
/// todo: Allow tests to pass in an enum to configure how the server behaves. (e.g. Echo vs. static.)
pub fn test_server() -> (SocketAddr, Receiver<Stats>) {
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