use std::io::prelude::*;
use std::net::{SocketAddr, SocketAddrV4, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};
use log::{error, info, warn};

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

            loop {
                let start = Instant::now();

                // stop the thread if we receive anything from the close receiver
                match rcv.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        info!("Closing child thread");
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                };

                match stream.write(msg.bytes.as_slice()) {
                    Err(ref e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        // todo: Figure out a cleaner way to handle broken pipes
                        stream = TcpStream::connect(addr).expect("Failed to reconnect");
                        thread_metrics.reconnects += 1;
                    }
                    Err(e) => {
                        error!("Unexpected error: {}", e);
                        break;
                    }
                    Ok(_) => {
                        thread_metrics.requests += 1;
                    }
                }

                // only try to obey rate limits if we're keeping up with the intended pace
                let elapsed = Instant::now() - start;
                match delay {
                    Some(d) => {
                        spin_sleep::sleep(d - elapsed);
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
