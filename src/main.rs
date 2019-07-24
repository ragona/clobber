use std::io;
use std::net::Ipv4Addr;

use clap::{App, Arg, ArgMatches};
use std::io::Read;

pub mod tcp_client;

#[derive(Debug, Copy, Clone)]
pub struct ClobberSettings {
    num_threads: u16,
    target: Ipv4Addr,
    port: u16,
    rate: u64,
}

fn main() -> std::io::Result<()> {
    let cli = cli();
    let settings = ClobberSettings::new(cli.get_matches());

    //    let mut buf = vec![];
    //    let stdin = io::stdin().read_to_end(&mut buf);
    //    let payload = match stdin {
    //        Ok(len) => &buf,
    //        Err(_) => &tcp_client::DEFAULT_REQUEST,
    //    };

    tcp_client::clobber(&settings, String::from("GET / HTTP/1.1\n"));

    Ok(())
}

impl ClobberSettings {
    pub fn new(matches: ArgMatches) -> ClobberSettings {
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

        ClobberSettings {
            target,
            port,
            rate,
            num_threads,
        }
    }
}

fn cli() -> App<'static, 'static> {
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
                .required(true),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Port to connect to")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("rate")
                .short("r")
                .long("rate")
                .help("Limit to a particular rate per-second.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("threads")
                .long("threads")
                .help("Number of threads")
                .takes_value(true),
        )
}
