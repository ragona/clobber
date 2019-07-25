#[allow(unused_imports)]
use log::{info, warn};
use std::io::prelude::*;
use std::net::{SocketAddr, SocketAddrV4, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crate::ClobberSettings;
use crossbeam_channel::{Receiver, TryRecvError};

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

pub fn clobber(settings: &ClobberSettings, message: Message, close: Receiver<()>) {
    // If there is no defined rate, we'll go as fast as we can
    let delay = match settings.rate {
        0 => None,
        _ => Some(Duration::from_nanos(
            (1e9 as u64 / settings.rate) * settings.num_threads as u64,
        )),
    };

    let addr: SocketAddr = SocketAddrV4::new(settings.target, settings.port).into();
    let mut thread_handles = vec![];

    for _ in 0..settings.num_threads {
        let msg = message.clone();
        let rcv = close.clone();
        let mut thread_metrics = Metrics::new();

        thread_handles.push(thread::spawn(move || {
            // one connection per thread
            let mut stream = TcpStream::connect(addr).expect("Failed to connect");
            stream.set_nodelay(true).expect("Failed to set_nodelay");

            loop {
                match rcv.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        info!("Closing child thread");
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                };

                // track how long this request takes
                let start = Instant::now();
                // write our request
                match stream.write(msg.bytes.as_slice()) {
                    // some clients break the pipe after each request
                    Err(ref e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        stream = TcpStream::connect(addr).expect("Failed to reconnect");
                        thread_metrics.reconnects += 1;
                    }
                    Err(_) => {
                        eprintln!("Unexpected error");
                        break;
                    }
                    Ok(_) => {
                        thread_metrics.requests += 1;
                    }
                }

                // only try to obey rate limits if we're keeping up with the intended pace
                let elapsed = Instant::now() - start;
                if delay.is_some() && elapsed < delay.unwrap() {
                    spin_sleep::sleep(delay.unwrap() - elapsed);
                }
            }

            info!("{:?}", thread_metrics);
        }));
    }

    for handle in thread_handles {
        handle.join().unwrap();
    }
}
