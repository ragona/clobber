pub mod stats;
pub mod tcp;

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
