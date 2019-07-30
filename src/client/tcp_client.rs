use std::future::Future;
use std::io::{self, prelude::*, stdin};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, ToSocketAddrs};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};
use log::{debug, error, info, trace, warn};

use crate::{err_msg, Error, Result};
use failure::_core::pin::Pin;
use failure::_core::result::Result::Err;
use failure::_core::task::{Context, Poll};

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

/// Take three: Get rid of all the faux async shit in Connection, move it to good old fashioned
/// sync code inside a client clobber function. Give the memory a nice clean place to live, etc.
/// I think the pieces to keep are the Poll results; those are conceptually what we're looking
/// for.  
impl Client {
    //    pub fn clobber
}

#[derive(Debug, Copy, Clone)]
pub struct ClientSettings {
    pub connections: u16,
    pub num_threads: u16,
    pub target: Ipv4Addr,
    pub port: u16,
    pub rate: u64,
    // todo: Add duration of run
}

pub enum PollResult {
    Read(usize),
    Write(usize),
}

pub struct Connection {
    message: Message,
    stream: TcpStream,
    read_buffer: Vec<u8>,
    current_request: Request,
}

impl Connection {
    // todo: Mixing result types within a single module is confusing
    pub fn connect(addr: &SocketAddr, message: &Message) -> io::Result<Connection> {
        let addr = addr.clone();
        let message = message.clone();
        let current_request = Request::new();
        let read_buffer = Vec::with_capacity(1024);
        let stream = TcpStream::connect(addr)?; // todo: Exponential backoff

        stream.set_nodelay(true)?;
        stream.set_nonblocking(true)?;

        Ok(Connection {
            message,
            stream,
            read_buffer,
            current_request,
        })
    }

    fn poll(self: &mut Self) -> Poll<io::Result<()>> {
        if self.current_request.is_done() {
            self.current_request = Request::new();
        }

        if !self.current_request.write_done {
            return match self.write(&mut self.stream, &mut self.message.bytes) {
                Poll::Ready(_) => {
                    self.current_request.write_done = true;
                    Poll::Pending
                }
                Poll::Pending => Poll::Pending,
            };
        } else if !self.current_request.read_done {
            return match self.read(&mut self.stream, &mut self.read_buffer) {
                Poll::Ready(_) => {
                    self.current_request.read_done = true;
                    Poll::Ready(Ok(()))
                }
                Poll::Pending => Poll::Pending,
            };
        };

        unreachable!()
    }

    pub fn write(&mut self, stream: &mut TcpStream, buf: &mut [u8]) -> Poll<io::Result<()>> {
        match stream.write_all(buf) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    pub fn read(&mut self, stream: &mut TcpStream, buf: &mut [u8]) -> Poll<io::Result<()>> {
        match stream.read(buf) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

pub struct Request {
    write_done: bool,
    read_done: bool,
}

impl Request {
    pub fn new() -> Request {
        Request {
            write_done: false,
            read_done: false,
        }
    }

    pub fn is_done(&self) -> bool {
        self.read_done && self.write_done
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
        let n = stream.write(b"hello").expect("Failed to write");
        info!("wrote {} bytes", n);
        //        let n = stream.read_to_end(&mut bytes).expect("Failed to read");
        //        info!("read {} bytes", n);
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

        let mut client = Connection::connect(&test_addr(), &Message::default())?;

        client.poll()?; // write
        client.poll()?; // blocked read
        client.poll()?; // read

        // todo: This is not working as intended; read is getting indefinitely blocked

        Ok(())
    }
}
