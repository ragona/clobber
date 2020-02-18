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
//! # use clobber::{tcp, Config, ConfigBuilder};
//!
//! let message = b"GET / HTTP/1.1\r\nHost: localhost:8000\r\nConnection: close\r\n\r\n".to_vec();
//! let addr = "127.0.0.1:8000".parse().unwrap();
//! let config = ConfigBuilder::new(addr)
//!     .connections(10)
//!     .build();
//!
//! tcp::clobber(config, message).unwrap();
//! ```
//!
pub mod config;
pub mod stats;
pub mod util;

pub use config::{Config, ConfigBuilder};
pub use stats::Stats;

use fern;
use log::LevelFilter;
use futures::prelude::*;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Receiver, Sender};

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

pub async fn go(worker_count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let jobs = vec![Work::new(10), Work::new(10)];
    let (task_tx, task_rx) = channel(10);
    
    tokio::spawn(async move {
        generate_work(task_tx, jobs).await;
    });

    work(task_rx, worker_count).await;

    Ok(())
}

async fn generate_work(mut task_tx: Sender<Task>, mut jobs: Vec<Work>) {
    loop {
        // todo: receive results, reorder jobs on priority (or make jobs a heap)

        if !jobs.is_empty() {
            let job = jobs.last_mut().unwrap();
            match job.next() {
                Some(task) => {
                    if let Err(_) = task_tx.send(task).await {
                        println!("we broke");
                        break;
                    }
                }
                None => {
                    jobs.pop().unwrap();
                }
            }
        } else {
            // todo: This is sort of a race condition where if new work doesn't
            // show up before all potential new work is found from outstanding 
            // requests we'll drop out too early.
            break;
        }
    }
}

async fn work(task_rx: Receiver<Task>, worker_count: usize) {
    task_rx
        .map(|task| {
            async move {
                // pretend to do work
                tokio::time::delay_for(Duration::from_millis(100)).await;

                // return some results
                Output { val: task.0 }
            }
        })
        .buffered(worker_count)
        .for_each(|out| {
            async move {
                println!("completed {}", out.val)
            }
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    pub fn foo() {
        let work = Work::new(5);

        for i in work {
            dbg!(i);
        }
    }
}
