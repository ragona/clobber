use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::future::Future;

use failure::{Error, err_msg};
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Error>;

const HOST:&str = "127.0.0.1:8080";
const NUM_THREADS:u8 = 4;

fn connect() -> Result<()> {
    let mut stream = TcpStream::connect(HOST)?;

//    stream.write(b"foo")?;
    let mut buf =  String::new();

    stream.read_to_string(&mut buf)?;
    dbg!(buf);
    Ok(())
}


fn main() -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle_client(mut stream: TcpStream) {
        stream.write(b"foo").expect("Failed to write to stream");
    }

    // test server
    fn listen() -> Result<()> {
        let listener = TcpListener::bind(HOST)?;

        // accept connections and process them serially
        for stream in listener.incoming() {
            handle_client(stream?);
        }

        Ok(())
    }

    #[test]
    fn client_test() -> Result<()> {
        // dedicated test thread for server
        thread::spawn(move || {
            listen().expect("failed to listen");
        });

        // wait for active listener
        loop {
            match connect() {
                Err(_) => { thread::sleep(Duration::from_millis(1)); },
                _ => { break; }
            }
        };

        let mut threads = vec![];

        for _ in 0..NUM_THREADS {
            threads.push(thread::spawn(move || {
                connect().expect("failed to connect");
            }))
        }

        for thread in threads {
            thread.join().expect("Child thread failed");
        }

        Ok(())
    }
}