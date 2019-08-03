#![feature(async_await)]

#[allow(unused_imports)]
pub mod client;

use std::net::Ipv4Addr;

use clap::{App, Arg, ArgMatches};
use client::tcp_client::{self, ClientSettings};
use crossbeam_channel::Sender;
use log::{info, LevelFilter};
use std::io::{stdin, Read};
use std::thread;
use std::time::Duration;

pub use failure::{err_msg, Error};
pub type Result<T> = std::result::Result<T, Error>;

fn main() -> Result<()> {
    let cli = cli();
    let matches = cli.get_matches();
    let settings = settings_from_argmatches(&matches);

    //    let message = match optional_stdin() {
    //        Some(bytes) => Message { bytes },
    //        None => Message::default(),
    //    };

    let log_level = match &matches.occurrences_of("v") {
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        3 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Warn,
    };

    setup_logger(log_level)?;

    // this channel is for closing child threads
    // todo: restore this functionality
    let (sender, close) = crossbeam_channel::unbounded();

    // catch interrupt and gracefully shut down child threads
    std::thread::spawn(move || {
        shutdown(sender, settings.num_threads);
    });

    // todo: Add back a call to kick off tcp client

    Ok(())
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

fn settings_from_argmatches(matches: &ArgMatches) -> ClientSettings {
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

    ClientSettings {
        target,
        port,
        rate,
        num_threads,
        connections,
        duration: None,
        connect_timeout: 0,
    }
}

pub fn setup_logger(log_level: LevelFilter) -> Result<()> {
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

fn shutdown(closer: Sender<()>, num_threads: u16) {
    ctrlc::set_handler(move || {
        info!("Shutting down");
        for _ in 0..num_threads {
            closer
                .send(())
                .expect("Failed to send close message to child thread");
        }
    })
    .expect("Failed to set ctrlc handler");
}

fn optional_stdin() -> Option<Vec<u8>> {
    let (sender, receiver) = crossbeam_channel::unbounded();

    thread::spawn(move || {
        let sin = stdin();
        let mut bytes = vec![];

        sin.lock()
            .read_to_end(&mut bytes)
            .expect("Failed to read from stdin");

        sender.send(bytes).expect("Failed to send input");
    });

    thread::sleep(Duration::from_millis(1));

    match receiver.try_recv() {
        Ok(l) => Some(l),
        _ => None,
    }
}
