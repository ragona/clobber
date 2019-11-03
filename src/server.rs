//! Echo server for testing
//! todo: Util server to capture request and save to file
use std::thread;
use std::net::SocketAddr;

use async_std::io::{self, Read};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;

async fn process(stream: TcpStream) -> io::Result<()> {
    let (reader, writer) = &mut (&stream, &stream);
    let mut buf = [0_u8; 1024];
    let bytes_read = reader.read(&mut buf).await?;
    writer.write(&buf[0..bytes_read]).await?;
    Ok(())
}

/// Echo server for testing. I'd love to put this into a test module, but
/// I can't figure out how to share between unit tests, integration tests,
/// benchmarking, etc, without just making it part of the main module.
pub fn echo_server() -> io::Result<SocketAddr> {
    // bind, return the port that the OS auto-assigns
    let listener:TcpListener = task::block_on(async {
        TcpListener::bind("127.0.0.1:0").await
    })?;

    let addr = listener.local_addr()?;

    // spin off a thread to start listening
    thread::spawn(move || {
        task::block_on(async move {
            let mut incoming = listener.incoming();
            while let Some(stream) = incoming.next().await {
                let stream = stream.unwrap();
                task::spawn(async {
                    process(stream).await.unwrap();
                });
            }
        })
    });

    Ok(addr)
}
