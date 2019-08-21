use std::net::SocketAddr;
use std::thread;
use std::time::Duration;
use std::io::{stdin, Read};

use futures::executor;
use futures::prelude::*;

use crossbeam_channel::Receiver;

use crate::{Stats};

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

/// This is a bit of a weird way to do this, but I'm not sure what the better option is.
/// What this bit of code does is that it spins off a thread to listen to stdin, and then
/// sleeps for a moment to give that thread time to put something in the channel. It... works.
/// Surely there is a more idiomatic option though. ‾\_(ツ)_/‾
pub fn optional_stdin() -> Option<Vec<u8>> {
    let (sender, receiver) = std::sync::mpsc::channel();

    thread::spawn(move || {
        let sin = stdin();
        let mut bytes = vec![];

        sin.lock()
            .read_to_end(&mut bytes)
            .expect("Failed to read from stdin");

        sender.send(bytes).expect("Failed to send input");
    });

    thread::sleep(Duration::from_millis(1));

    match receiver.try_recv() {
        Ok(l) => Some(l),
        _ => None,
    }
}
