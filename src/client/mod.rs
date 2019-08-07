pub mod stats;
pub mod tcp_client;

pub use stats::Stats;
pub use tcp_client::Config;

#[derive(Debug, Clone)]
pub struct Message {
    pub body: Vec<u8>,
}

impl Message {
    pub fn new(body: Vec<u8>) -> Message {
        Message { body }
    }
}
