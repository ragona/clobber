#[allow(unused_imports)]
pub mod client;

use std::io::{stdin, Read};
use std::net::Ipv4Addr;

use clap::{App, Arg, ArgMatches};
use crossbeam_channel::Sender;
use log::{info, LevelFilter};

use client::tcp_client::{self, Message};

pub use failure::Error;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Copy, Clone)]
pub struct ClobberSettings {
    connections: u16,
    num_threads: u16,
    target: Ipv4Addr,
    port: u16,
    rate: u64,
    // todo: Add duration of run
}

fn main() -> Result<()> {
    let cli = cli();
    let matches = cli.get_matches();

    match matches.occurrences_of("v") {
        1 => setup_logger(log::LevelFilter::Info).expect("Failed to setup logger"),
        2 => setup_logger(log::LevelFilter::Debug).expect("Failed to setup logger"),
        3 => setup_logger(log::LevelFilter::Trace).expect("Failed to setup logger"),
        _ => setup_logger(log::LevelFilter::Warn).expect("Failed to setup logger"),
    }

    let settings = ClobberSettings::new(matches);

    // this channel is for closing child threads
    let (sender, receiver) = crossbeam_channel::unbounded();

    // catch interrupt and gracefully shut down child threads
    std::thread::spawn(move || {
        shutdown(sender, settings);
    });

    tcp_client::clobber(settings, build_message(settings), receiver)?;

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

        let connections = matches
            .value_of("connections")
            .unwrap_or("10")
            .parse::<u16>()
            .expect("Failed to parse connections");

        ClobberSettings {
            target,
            port,
            rate,
            num_threads,
            connections,
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
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
}

fn setup_logger(log_level: LevelFilter) -> Result<()> {
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
        .level(log_level)
        .chain(std::io::stdout())
        .chain(fern::log_file("clobber.log")?)
        .apply()?;

    Ok(())
}

fn build_message(_settings: ClobberSettings) -> Message {
    let mut lines: Vec<u8> = vec![];
    match stdin().read_to_end(&mut lines) {
        Ok(_) => {}
        Err(_) => {
            lines.append(&mut b"GET /".to_vec());
        }
    }

    Message::new(lines)
}

fn shutdown(closer: Sender<()>, settings: ClobberSettings) {
    ctrlc::set_handler(move || {
        info!("Shutting down");
        for _ in 0..settings.num_threads {
            closer
                .send(())
                .expect("Failed to send close message to child thread");
        }
    })
    .expect("Failed to set ctrlc handler");
}
