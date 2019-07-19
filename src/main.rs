#![feature(async_await)]

use std::io;
use std::thread;

use futures::executor::{self, ThreadPool};
use futures::io::{AllowStdIo, AsyncReadExt, AsyncWriteExt};
use futures::task::SpawnExt;
use futures::StreamExt;

use romio::{TcpListener, TcpStream};
use std::time::Duration;

const HOST: &str = "127.0.0.1:8000";
const REQUEST: &[u8] = b"GET /";

fn main() -> io::Result<()> {
    executor::block_on(async {

        let mut threadpool = ThreadPool::new().unwrap();

        for i in 0..10 {
            threadpool
                .spawn(async move {
                    let mut stream = TcpStream::connect(&"127.0.0.1:8000".parse().unwrap()).await;

                    match stream {
                        Ok(mut s) => {
                            s.write_all(&REQUEST).await;
                        }
                        Err(e) => {
                            dbg!(e);
                        }
                    }
                })
                .unwrap();
        }
    });



//    thread::sleep(Duration::from_millis(500)); // how to block appropriately?

    Ok(())
}
//    executor::block_on(async {
//        let mut threadpool = ThreadPool::new()?;
//
//        for i in 0..10 {
//
//            threadpool
//                .spawn(async move {
//                    let mut stream = TcpStream::connect(&HOST.parse().unwrap());
//                    stream.write(&REQUEST).await?;
//                })
//                .unwrap();
//        }
//
//        Ok(())
//    })