use std::io::prelude::*;
use std::net::{SocketAddr, SocketAddrV4, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crate::ClobberSettings;

#[derive(Clone)]
pub struct Message {
    bytes: Vec<u8>,
}

impl Message {
    pub fn new(bytes: Vec<u8>) -> Message {
        Message { bytes }
    }
}

pub fn clobber(settings: &ClobberSettings, message: Message) {
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
        // Each thread should be able to modify its own message, so it needs its own copy
        let msg = message.clone();

        thread_handles.push(thread::spawn(move || {
            // one connection per thread
            let mut stream = TcpStream::connect(addr).expect("Failed to connect");
            stream.set_nodelay(true).expect("Failed to set_nodelay");

            loop {
                // track how long this request takes
                let start = Instant::now();
                // write our request
                match stream.write(msg.bytes.as_slice()) {
                    // some clients break the pipe after each request
                    Err(ref e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        stream = TcpStream::connect(addr).expect("Failed to reconnect");
                    }
                    Err(_) => {
                        eprintln!("Unexpected error");
                        break;
                    }
                    _ => {}
                }

                // only try to obey rate limits if we're keeping up with the intended pace
                let elapsed = Instant::now() - start;
                if delay.is_some() && elapsed < delay.unwrap() {
                    spin_sleep::sleep(delay.unwrap() - elapsed);
                }
            }
        }));
    }

    for handle in thread_handles {
        handle.join().unwrap();
    }
}
