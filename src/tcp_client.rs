use std::io::prelude::*;
use std::net::{SocketAddr, SocketAddrV4, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};
use log::{debug, error, info, trace, warn};

use crate::ClobberSettings;

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

pub fn clobber(settings: ClobberSettings, message: Message, close: Receiver<()>) {
    let delay = match settings.rate {
        0 => None,
        _ => Some(Duration::from_nanos(1e9 as u64 / settings.rate) * settings.num_threads as u32),
    };

    let addr: SocketAddr = SocketAddrV4::new(settings.target, settings.port).into();
    let mut thread_handles = vec![];

    for _ in 0..settings.num_threads {
        let msg = message.clone();
        let rcv = close.clone();
        let mut thread_metrics = Metrics::new();

        // sleep here to stagger worker threads
        spin_sleep::sleep(Duration::from_millis(1000 / settings.num_threads as u64));

        thread_handles.push(thread::spawn(move || {
            // todo: don't panic on connection failure
            let mut stream = TcpStream::connect(addr).expect("Failed to connect");
            let mut read_buf = [0u8; 65535];

            loop {
                let start = Instant::now();

                // stop the thread if we receive anything from the close receiver
                match rcv.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                };

                // I'm using this to try to see if the connection is open. I have no idea
                // whether this is a good idea or not.
                match stream.write(&mut []) {
                    Err(ref e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        // todo: exponential backoff reconnect
                        // todo: don't panic
                        stream = TcpStream::connect(addr).expect("Failed to reconnect");
                        thread_metrics.reconnects += 1;
                        debug!("reconnect");
                    }
                    Ok(_) => trace!("test write succeeded"),
                    Err(e) => warn!("test write failed with error: {}", e),
                }

                // todo: Handle partial writes
                match stream.write(msg.bytes.as_slice()) {
                    Err(e) => {
                        error!("Unexpected error: {}", e);
                        break;
                    }
                    Ok(x) => {
                        trace!("write complete {}", x);
                        thread_metrics.requests += 1;
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

                // only try to obey rate limits if we're keeping up with the intended pace
                let elapsed = Instant::now() - start;
                match delay {
                    Some(delay) => {
                        if elapsed < delay {
                            spin_sleep::sleep(delay - elapsed);
                        }
                    }
                    None => {}
                }
            }

            info!("{:?}", thread_metrics);
        }));
    }

    for handle in thread_handles {
        handle.join().unwrap();
    }
}
