//! # Tool for TCP load testing
//!
//! The primary goal for `clobber` is speed; we want to make TCP requests as fast as possible. If
//! you're interested in reading the code, go check out `tcp.rs` for the interesting part!
//!
//! This library is used internally by the main.rs binary and the tests, and is not intended for
//! general use in other projects. (But if you're interested, post an issue; I'd be happy to hear
//! about it!)
//!
//! ## Examples
//!
//! ```no_run
//! # use std::time::Duration;
//! # use clobber::{self, Config, ConfigBuilder};
//!
//! let message = b"GET / HTTP/1.1\r\nHost: localhost:8000\r\nConnection: close\r\n\r\n".to_vec();
//! let addr = "127.0.0.1:8000".parse().unwrap();
//! let config = ConfigBuilder::new(addr)
//!     .workers(10)
//!     .build();
//!
//! // clobber::go(config, message).await?;
//! ```
//!
pub mod config;
pub mod stats;
pub mod tcp;
pub mod util;

pub use config::{Config, ConfigBuilder};
pub use stats::Stats;

use byte_mutator::fuzz_config::FuzzConfig;
use byte_mutator::ByteMutator;
use crossbeam_channel::{bounded, Receiver, Sender};
use fern;
use futures::prelude::*;
use log::LevelFilter;
use std::time::Duration;

pub fn setup_logger(log_level: LevelFilter) -> Result<(), Box<dyn std::error::Error>> {
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

#[derive(Debug)]
pub struct Work {
    cur: usize,
    amount: usize,
}

impl Work {
    pub fn new(amount: usize) -> Self {
        Self { cur: 0, amount }
    }
}

#[derive(Debug)]
pub struct Output {
    pub val: usize,
}

pub struct Analysis {}

#[derive(Debug)]
pub struct Task(pub usize);

impl From<Task> for usize {
    fn from(task: Task) -> usize {
        task.0
    }
}

impl Iterator for Work {
    type Item = Task;

    fn next(&mut self) -> Option<Task> {
        let n = self.cur;
        self.cur += 1;

        match n {
            n if n <= self.amount => Some(Task(n)),
            _ => None,
        }
    }
}

pub async fn go(config: Config, message: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
    // let message = match &config.fuzz_path {
    //     None => ByteMutator::new(&message),
    //     Some(path) => match FuzzConfig::from_file(&path) {
    //         Ok(fuzz_config) => ByteMutator::new_from_config(&message, fuzz_config),
    //         Err(e) => return Err(e.into()),
    //     },
    // };

    // create channels

    // start tcp workers

    // start generating work

    // analyze results

    // see if it's time to stop

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    pub fn foo() {
        //
    }
}
