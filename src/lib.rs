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
//! # use clobber::{tcp, Message, Config, ConfigBuilder};
//!
//! let addr = "127.0.0.1:8000".parse().unwrap();
//! let message = Message::new(b"GET / HTTP/1.1\r\nHost: localhost:8000\r\nConnection: close\r\n\r\n");
//! let config = ConfigBuilder::new(addr)
//!     .connections(10)
//!     .build();
//!
//! tcp::clobber(config, message).unwrap();
//! ```
//!
pub mod config;
pub mod server;
pub mod stats;
pub mod tcp;
pub mod util;

pub use config::{Config, ConfigBuilder};
pub use stats::Stats;

use byte_mutator::undo_buffer::UndoBuffer;
use fern;
use log::LevelFilter;

/// Message payload
///
/// todo: Long-term goal; provide APIs for each connection to mutate its message.
///
#[derive(Debug, Clone)]
pub struct Message {
    pub body: UndoBuffer,
}

impl Message {
    pub fn new(bytes: &[u8]) -> Message {
        Message {
            body: UndoBuffer::new(bytes),
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
}
