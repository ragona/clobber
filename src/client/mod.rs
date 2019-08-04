pub mod tcp_client;

pub struct Message {
    pub body: Vec<u8>,
}

impl Message {
    pub fn new(body: Vec<u8>) -> Message {
        Message { body }
    }
}
