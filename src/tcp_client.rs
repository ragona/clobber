use std::io::prelude::*;
use std::net::{SocketAddr, SocketAddrV4, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crate::ClobberSettings;

pub const DEFAULT_REQUEST: &'static [u8] = b"GET / HTTP/1.1
Host: localhost:8000
User-Agent: clobber
Accept: */*\n
";

pub fn clobber(settings: ClobberSettings) {
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
        thread_handles.push(thread::spawn(move || {
            // one connection per thread
            let mut stream = TcpStream::connect(addr).expect("Failed to connect");
            stream.set_nodelay(true).expect("Failed to set_nodelay");

            loop {
                // track how long this request takes
                let start = Instant::now();
                // write our request
                match stream.write(settings.payload) {
                    Ok(_) => (),
                    // some clients break the pipe after each request
                    Err(ref e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        stream = TcpStream::connect(addr).expect("Failed to reconnect");
                    }
                    Err(_) => {
                        eprintln!("Unexpected error");
                        break;
                    }
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
