use std::ops::Add;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Copy, Clone)]
pub struct Stats {
    pub bytes_read: usize,
    pub bytes_written: usize,
    pub connections: usize,
    pub connection_attempts: usize,
    pub start_time: Instant,
    pub end_time: Instant,
}

impl Stats {
    pub fn duration(self: &Self) -> Duration {
        self.end_time - self.start_time
    }
}

impl Stats {
    pub fn new() -> Stats {
        Stats {
            bytes_read: 0,
            bytes_written: 0,
            connections: 0,
            connection_attempts: 0,
            start_time: Instant::now(),
            end_time: Instant::now(),
        }
    }
}

impl Add for Stats {
    type Output = Stats;

    fn add(self: Stats, other: Stats) -> Stats {
        Stats {
            bytes_read: self.bytes_read + other.bytes_read,
            bytes_written: self.bytes_written + other.bytes_written,
            connections: self.connections + other.connections,
            connection_attempts: self.connection_attempts + other.connection_attempts,
            start_time: self.start_time,
            end_time: other.end_time, // todo: Is this right?
        }
    }
}
