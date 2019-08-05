#![feature(async_await)]

#[allow(unused_imports)]
pub mod client;

use std::net::Ipv4Addr;

use crate::client::{tcp_client, Message};
use clap::{App, Arg, ArgMatches};
use client::tcp_client::Config;
pub use failure::{err_msg, Error};
use log::{info, LevelFilter};
use std::io::{stdin, Read};
use std::thread;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Error>;

fn main() -> Result<()> {
    let cli = cli();
    let matches = cli.get_matches();
    let settings = settings_from_argmatches(&matches);

    let log_level = match &matches.occurrences_of("v") {
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        3 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Warn,
    };

    setup_logger(log_level)?;

    let bytes = match optional_stdin() {
        Some(bytes) => bytes,
        None => unimplemented!("no request body"), // todo: Load from file
    };

    let message = Message::new(bytes);

    // catch interrupt and gracefully shut down child threads
    //    std::thread::spawn(move || {
    //        shutdown(close_sender);
    //    });

    tcp_client::clobber(settings, message)?;

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
        .arg(
            Arg::with_name("connect-timeout")
                .long("connect-timeout")
                .help("Timeout for initial TCP syn timeout")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("read-timeout")
                .long("read-timeout")
                .help("Timeout for reading data from target")
                .takes_value(true),
        )
}

fn settings_from_argmatches(matches: &ArgMatches) -> Config {
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
        .parse::<usize>()
        .expect("Failed to parse rate");

    let num_threads = matches
        .value_of("threads")
        .unwrap_or("4")
        .parse::<u16>()
        .expect("Failed to parse number of threads");

    let connect_timeout = matches
        .value_of("connect-timeout")
        .unwrap_or("500")
        .parse::<u32>()
        .expect("Failed to parse connect-timeout");

    let read_timeout = matches
        .value_of("read-timeout")
        .unwrap_or("500")
        .parse::<u32>()
        .expect("Failed to parse read-timeout");

    Config {
        target,
        port,
        rate,
        num_threads,
        duration: None, // todo: add human-duration duration value
        connect_timeout,
        read_timeout,
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

/// This is a bit of a weird way to do this, but I'm not sure what the better option is.
/// What this bit of code does is that it spins off a thread to listen to stdin, and then
/// sleeps for a moment to give that thread time to put something in the channel. It... works.
/// Surely there is a more idiomatic option though. ‾\_(ツ)_/‾
fn optional_stdin() -> Option<Vec<u8>> {
    let (sender, receiver) = std::sync::mpsc::channel();

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
