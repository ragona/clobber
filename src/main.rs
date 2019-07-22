use std::io::prelude::*;
use std::net::{TcpStream, Ipv4Addr, SocketAddrV4, SocketAddr};
use std::thread;

use clap::{App, Arg, ArgMatches};
use std::time::{Duration, Instant};
use spin_sleep;

pub mod util;

const REQUEST: &'static [u8] = b"GET / HTTP/1.1
Host: localhost:8000
User-Agent: clobber
Accept: */*\n
";


fn cli() -> ArgMatches<'static> {
    App::new("clobber")
        .version("0.1")
        .author("ryan ragona <ryan@ragona.com>")
        .about("tcp load testing tool")
        .arg(
            Arg::with_name("target")
                .short("t")
                .long("target")
                .help("Host to clobber")
                .takes_value(true)
                .required(true)
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Port to connect to")
                .takes_value(true)
                .required(true)
        )
        .arg(
            Arg::with_name("rate")
                .short("r")
                .long("rate")
                .help("Limit to a particular rate per-second.")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("threads")
                .long("threads")
                .help("Number of threads")
                .takes_value(true)
        )
        .get_matches()
}


fn main() -> std::io::Result<()> {
    let matches = cli();

    let target = matches
        .value_of("target")
        .expect("Failed to parse target")
        .parse::<Ipv4Addr>()
        .expect("Failed to parse target");

    let port = matches
        .value_of("port")
        .unwrap_or("80")
        .parse::<u16>()
        .expect("Failed to parse port");

    let rate = matches
        .value_of("rate")
        .unwrap_or("0")
        .parse::<u64>()
        .expect("Failed to parse rate");

    let num_threads = matches
        .value_of("threads")
        .unwrap_or("4")
        .parse::<u16>()
        .expect("Failed to parse number of threads");


    // If there is no defined rate, we'll go as fast as we can
    let delay = match rate {
        0 => None,
        _ => {
            Some(Duration::from_nanos((1e9 as u64 / rate) * num_threads as u64))
        }
    };

    let addr:SocketAddr = SocketAddrV4::new(target, port).into();

    let mut thread_handles = vec![];

    for _ in 0..num_threads {
        thread_handles.push(thread::spawn(move || {
            // one connection per thread
            let mut stream = TcpStream::connect(addr).expect("Failed to connect");
            stream.set_nodelay(true).expect("Failed to set_nodelay");

            loop {
                // track how long this request takes
                let start = Instant::now();
                // write our request
                match stream.write(REQUEST) {
                    Ok(_) => (),
                    // try to reconnect on failure
                    Err(ref e) if e.kind() == std::io::ErrorKind::BrokenPipe  => {
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

    Ok(())
}
