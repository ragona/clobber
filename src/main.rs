#![feature(async_await)]

use std::io;
use std::thread;
use std::time::Duration;

use futures::executor;
use futures::io::{AllowStdIo, AsyncReadExt, AsyncWriteExt};

use romio::TcpStream;
use juliex;

const HOST: &str = "127.0.0.1:8000";
const REQUEST: &[u8] = b"GET /";
const REQUESTS_PER_SECOND: u64 = 1000;
const TOTAL_REQUESTS: u64 = 10000;

fn main() -> io::Result<()> {
    let delay = 1e9 as u64 / REQUESTS_PER_SECOND;

    executor::block_on(async {
        for _ in 0..TOTAL_REQUESTS {
            juliex::spawn(async move {
                let stream = TcpStream::connect(&HOST.parse().unwrap()).await;
                match stream {
                    Ok(mut s) => {
                        s.write_all(&REQUEST).await.expect("Failed to write to socket");
                        s.close().await.expect("Failed to close socket");
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            });

            thread::sleep(Duration::from_nanos(delay));
        }
    });

    Ok(())
}


