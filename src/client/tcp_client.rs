use std::io::prelude::*;
use std::io::stdin;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, ToSocketAddrs};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};
use log::{debug, error, info, trace, warn};

use crate::{err_msg, Error, Result};
use failure::_core::result::Result::Err;

#[derive(Debug, Clone)]
pub struct Message {
    pub bytes: Vec<u8>,
}

impl Message {
    pub fn new() -> Message {
        Message { bytes: vec![] }
    }

    pub fn default() -> Message {
        Message {
            bytes: b"GET / HTTP/1.1\r\n".to_vec(),
        }
    }
}

#[derive(Debug)]
pub struct Client {
    settings: ClientSettings,
    message: Message,
    close: Receiver<()>,
}

impl Client {}

#[derive(Debug, Copy, Clone)]
pub struct ClientSettings {
    pub connections: u16,
    pub num_threads: u16,
    pub target: Ipv4Addr,
    pub port: u16,
    pub rate: u64,
    // todo: Add duration of run
}

pub struct Connection {
    message: Message,
    stream: TcpStream,
    current_request: Request,
}

impl Connection {
    pub fn connect(addr: &SocketAddr, message: &Message) -> Result<Connection> {
        let addr = addr.clone();
        let message = message.clone();
        let current_request = Request::new();
        let stream = TcpStream::connect(addr)?; // todo: Exponential backoff

        stream.set_nodelay(true)?;
        stream.set_nonblocking(true)?;

        Ok(Connection {
            message,
            stream,
            current_request,
        })
    }

    pub fn poll(&mut self) -> Result<()> {
        // Each connection is a worker, so it should have a queue of its own.
        // I guess it has some max number it works on at once? No, only one at once?
        // Yeah, only one at once per connection or we start getting out of order responses.
        // So... each thread loops through all of its connections and polls them. Each
        // connection then polls its active request. If the active request is complete, it
        // pops from its work queue and starts working on the new request.
        if self.current_request.is_done() {
            self.current_request = Request::new();
        }

        if !self.current_request.write_done {
            match self
                .current_request
                .write(&mut self.stream, &mut self.message.bytes)
            {
                Ok(_) => return Ok(()),
                Err(_) => {} // todo: Not sure how to handle this err
            };
        } else if !self.current_request.read_done {
            match self.current_request.read(&mut self.stream) {
                Ok(_) => {}
                Err(_) => {}
            }
        }

        Ok(())
    }
}

pub struct Request {
    write_done: bool,
    read_done: bool,
    buf: [u8; 1024],
}

impl Request {
    pub fn new() -> Request {
        Request {
            write_done: false,
            read_done: false,
            buf: [0; 1024],
        }
    }

    pub fn is_done(&self) -> bool {
        self.read_done && self.write_done
    }

    // todo: Would this be faster without using failure types here? Maybe it just can't fail?
    pub fn write(&mut self, stream: &mut TcpStream, buf: &mut [u8]) -> Result<()> {
        // I'm assuming that stream will be set to nonblocking, and all I'm trying to
        // signal here is that Ok or WouldBlock are fine (and instant) responses, and
        // I'm not sure what panics I'll see. todo: Fix
        match stream.write_all(buf) {
            Ok(_) => {
                info!("write done {}", buf.len());
                self.write_done = true;
                Ok(())
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                info!("blocked write");
                return Err(err_msg("blocked"));
            }
            Err(e) => panic!("Unhandled write error: {}", e),
        }
    }

    pub fn read(&mut self, stream: &mut TcpStream) -> Result<()> {
        match stream.read(&mut self.buf) {
            Ok(_) => {
                info!("read done");
                self.read_done = true;
                Ok(())
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                info!("blocked read");
                return Err(err_msg("blocked"));
            }
            Err(e) => panic!("encountered IO error: {}", e),
        }
    }
}

/**

Idea for lightweight concurrency model: Add number of connections as well as number of
threads, and enable non-blocking TCP. Distribute connections per thread. Each thread
loops through its connections and performs the non-blocking read or write from the socket
associated with that connection. If the latency is low then the overall number of
connections per OS thread should be fairly small, and if it starts backing up it's still
limited overall by the number of connections, which is divided evenly by the active
threads. A work stealing algorithm might smooth this out in case a single thread ends
up with more than its fair share of poor connections, we'll see if that's necessary.

**/

fn limit_rate(start: &Instant, delay: &Option<Duration>) {
    let elapsed = Instant::now() - *start;
    match delay {
        Some(delay) => {
            if elapsed < *delay {
                spin_sleep::sleep(*delay - elapsed);
            }
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_logger;
    use std::net::TcpListener;

    // todo: Make this observable
    fn start_fake_server() -> Result<()> {
        setup_logger(log::LevelFilter::Trace).unwrap();

        thread::spawn(move || {
            let listener = TcpListener::bind(test_addr()).expect("Fake server failed to bind");
            for stream in listener.incoming() {
                info!("incoming connection");
                handle_client(&mut stream.expect("Failed to unwrap stream"));
            }
        });

        thread::sleep(Duration::from_millis(1));

        Ok(())
    }

    fn handle_client(stream: &mut TcpStream) {
        info!("handling client");
        let mut bytes = vec![];
        let n = stream.read_to_end(&mut bytes).expect("Failed to read");
        info!("read {} bytes", n);
        let n = stream.write(b"hello").expect("Failed to write");
        info!("wrote {} bytes", n);
    }

    fn test_addr() -> SocketAddr {
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let port = 8888;
        let addr = SocketAddrV4::new(ip, port);

        addr.into()
    }

    #[test]
    fn test_connect() -> Result<()> {
        start_fake_server()?;

        Connection::connect(&test_addr(), &Message::default())?;

        Ok(())
    }

    #[test]
    fn test_write() -> Result<()> {
        start_fake_server()?;

        let mut connection = Connection::connect(&test_addr(), &Message::default())?;

        connection.poll()?;

        Ok(())
    }

    #[test]
    fn test_read() -> Result<()> {
        start_fake_server()?;

        let mut connection = Connection::connect(&test_addr(), &Message::default())?;

        for _ in 0..10 {
            connection.poll()?;
            thread::sleep(Duration::from_millis(5));
        }

        // todo: This is not working as intended; read is getting indefinitely blocked

        Ok(())
    }
}
