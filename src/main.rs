use std::net::Ipv4Addr;

use clap::{App, Arg, ArgMatches};

pub mod tcp_client;

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
        .get_matches()
}

pub struct ClobberSettings {
    target: Ipv4Addr,
    port: u16,
    rate: u64,
    num_threads: u16,
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

    tcp_client::clobber(ClobberSettings {
        rate,
        port,
        target,
        num_threads,
    });

    Ok(())
}
