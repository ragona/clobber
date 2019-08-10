//! Tool for TCP load testing
//!
//! The primary goal for `clobber` is speed; we want to make TCP requests as fast as possible.
//!
//! This library is used by the main.rs binary and the tests.
//!
//! # Examples
//!
//! ## A single thread
//!
//! ```no_run
//! # use std::time::Duration;
//! # use clobber::{tcp, Message, Config};
//!
//! let addr = "127.0.0.1:8000".parse().unwrap();
//! let config = Config::new(addr, 10);
//! let message = Message::new(b"GET / HTTP/1.1\r\nHost: localhost:8000\r\nConnection: close\r\n\r\n".to_vec());
//!
//! tcp::clobber(config, message).unwrap();
//! ```

#![feature(async_await)]

pub mod stats;
pub mod tcp;
pub mod util;

pub use stats::Stats;
pub use tcp::Config;

#[derive(Debug, Clone)]
pub struct Message {
    pub body: Vec<u8>,
}

impl Message {
    pub fn new(body: Vec<u8>) -> Message {
        Message { body }
    }
}
