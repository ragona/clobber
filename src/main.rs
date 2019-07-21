#![feature(async_await)]

use std::io;
use std::thread;
use std::net::{SocketAddr, Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use futures::executor;
use futures::io::{AllowStdIo, AsyncReadExt, AsyncWriteExt};

use romio::TcpStream;
use juliex;

use rand::Rng;

pub mod util;

const HOST: &str = "172.31.33.10:7878";
const TOTAL_REQUESTS: u64 = 1000;
const REQUESTS_PER_SECOND: u64 = 2;
const REQUEST: &[u8] = b"GET / HTTP/1.1
User-Agent: clobber
Accept: */*\n
";

fn main() -> io::Result<()> {
    let delay = 1e9 as u64 / REQUESTS_PER_SECOND;

    executor::block_on(async {

        for _ in 0..TOTAL_REQUESTS {
            juliex::spawn(async move {
                let addr = util::random_ipv4_addr(80);

                println!("Connecting to: {:?}", &addr);

                match TcpStream::connect(&addr.into()).await {
                    Ok(mut stream) => {
                        println!("Writing: {:?}", &addr);

                        stream.write_all(&REQUEST)
                            .await
                            .expect("Failed to write to socket");

                        stream.close()
                            .await
                            .expect("Failed to close socket");

                        println!("Success: {:?}", &addr);
                    }
                    Err(e) => {
                        eprintln!("Failed to connect: '{}'", e);
                    }
                }

            });

            thread::sleep(Duration::from_nanos(delay));
        }
    });

    Ok(())
}
