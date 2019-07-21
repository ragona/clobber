#![feature(async_await)]

use std::io;
use std::thread;
use std::time::Duration;

use futures::executor;
use futures::io::{AllowStdIo, AsyncReadExt, AsyncWriteExt};

use romio::TcpStream;
use juliex;
use futures::lock::Mutex;
use std::sync::Arc;

pub mod util;

const HOST: &str = "127.0.0.1:7878";
const TOTAL_REQUESTS: u64 = 1000000000;
const REQUESTS_PER_SECOND: u64 = 25000;
const REQUEST: &[u8] = b"GET / HTTP/1.1
User-Agent: clobber
Accept: */*\n
";


fn main() -> io::Result<()> {
    let delay = 1e9 as u64 / REQUESTS_PER_SECOND;
    let addr = HOST.parse().expect("Failed to parse host");

    executor::block_on(async {
        match TcpStream::connect(&addr).await {
            Ok(mut stream) => {
                for _ in 0..TOTAL_REQUESTS {
                    stream.write_all(&REQUEST)
                        .await
                        .expect("Failed to write to socket");
                }

                stream.close()
                    .await
                    .expect("Failed to close socket");
            }
            Err(e) => {
                eprintln!("Failed to connect to '{}': '{:?}'", &addr, e);
            }
        }
    });

    Ok(())
}
