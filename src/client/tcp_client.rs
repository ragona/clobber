use std::io::prelude::*;
use std::net::{SocketAddr, SocketAddrV4, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};
use log::{debug, error, info, trace, warn};

use crate::{ClobberSettings, Error, Result};
use std::thread::JoinHandle;

#[derive(Debug, Clone)]
pub struct Message {
    bytes: Vec<u8>,
}

impl Message {
    pub fn new(bytes: Vec<u8>) -> Message {
        Message { bytes }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Metrics {
    requests: u64,
    connects: u64,
    reconnects: u64,
}

impl Metrics {
    pub fn new() -> Metrics {
        Metrics {
            requests: 0,
            connects: 0,
            reconnects: 0,
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

pub fn clobber(settings: ClobberSettings, message: Message, close: Receiver<()>) -> Result<()> {
    let addr: SocketAddr = SocketAddrV4::new(settings.target, settings.port).into();
    let mut thread_handles: Vec<JoinHandle<Result<()>>> = vec![];

    for _ in 0..settings.num_threads {
        // todo: figure out why I can't clone these inline with the function invocation
        let msg = message.clone();
        let cls = close.clone();

        thread_handles.push(thread::spawn(move || {
            sub_clobber(addr.clone(), msg, cls, settings.clone())
        }));

        // try to evenly stagger thread starts
        spin_sleep::sleep(Duration::from_millis(1000 / settings.num_threads as u64));
    }

    for handle in thread_handles {
        handle
            .join()
            .expect("Failed to join on child thread")
            .expect("Child thread failed to return");
    }

    Ok(())
}

// todo: turn this into a struct with an implementation
fn sub_clobber(
    addr: SocketAddr,
    message: Message,
    close: Receiver<()>,
    settings: ClobberSettings,
) -> Result<()> {
    let mut stream = connect_and_configure(addr)?;
    let mut read_buf = [0u8; 65535];
    let delay = match settings.rate {
        0 => None,
        _ => Some(Duration::from_nanos(1e9 as u64 / settings.rate) * settings.num_threads as u32),
    };

    while !close_requested(&close) {
        let start = Instant::now();

        // todo: Handle partial writes
        match stream.write(message.bytes.as_slice()) {
            Err(e) => {
                error!("Unexpected error: {}", e);
                break;
            }
            Ok(x) => {
                trace!("write complete {}", x);
            }
        }

        loop {
            match stream.read(&mut read_buf) {
                Ok(0) => break,
                Ok(n) => trace!("read {} bytes", n),
                Err(e) => {
                    warn!("{}", e);
                    break;
                }
            }
        }

        limit_rate(&start, &delay);
    }

    info!("child thread closed");

    Ok(())
}

fn connect_and_configure(addr: SocketAddr) -> Result<TcpStream> {
    let stream = TcpStream::connect(addr)?;

    stream.set_nodelay(true)?;
    stream.set_nonblocking(true)?;

    Ok(stream)
}

fn close_requested(close: &Receiver<()>) -> bool {
    match close.try_recv() {
        Ok(_) | Err(TryRecvError::Disconnected) => true,
        Err(TryRecvError::Empty) => false,
    }
}

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
