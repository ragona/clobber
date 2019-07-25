#[allow(unused_imports)]
use std::io::{stdin, Read};
use std::net::Ipv4Addr;

use clap::{App, Arg, ArgMatches};

use log::{info, warn};

use crate::tcp_client::Message;

pub mod tcp_client;

#[derive(Debug, Copy, Clone)]
pub struct ClobberSettings {
    num_threads: u16,
    target: Ipv4Addr,
    port: u16,
    rate: u64,
    // todo: Add duration of run
}

fn main() -> std::io::Result<()> {
    // startup the cli first or stdin will block
    let cli = cli();
    let settings = ClobberSettings::new(cli.get_matches());

    setup_logger().expect("Failed to setup logger");

    let (sender, receiver) = crossbeam_channel::unbounded();

    // catch ctrl and gracefully shut down child threads
    std::thread::spawn(move || {
        ctrlc::set_handler(move || {
            info!("Shutting down");
            for _ in 0..settings.num_threads {
                sender
                    .send(())
                    .expect("Failed to send close message to child thread");
            }
        })
    });

    // read from stdin todo: Add option to give file path
    let mut lines: Vec<u8> = vec![];
    stdin().read_to_end(&mut lines).unwrap();
    let message = Message::new(lines);

    // run until interrupt todo: add graceful ctrl + c
    tcp_client::clobber(&settings, message, receiver);

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

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(fern::log_file("clobber.log")?)
        .apply()?;
    Ok(())
}
