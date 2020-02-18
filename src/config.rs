//! Settings that control the test
//!
//! This is a struct and impl that contains the configuration options. `Config` should be easy
//! to copy, as it is passed to each `connection()` call.
//!

use std::net::SocketAddr;
use std::time::Duration;

use serde_derive::Deserialize;

/// Settings for the load test
///
/// todo: Make write/read optional. (Enum?)
///
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Socket address (ip and port) of the host we'll be calling
    pub target: SocketAddr,
    /// Number of active workers. (Note: consider max number of open ports and files when setting this)
    pub workers: u32,
    /// Optional rate-limiting. Precisely timing high rates is unreliable; if you're
    /// seeing slower than expected performance try running with no rate limit at all.
    pub rate: Option<u32>,
    /// Optional duration. If duration is None, clobber will run indefinitely.
    pub duration: Option<Duration>,
    /// Number of OS threads to distribute work between. 0 becomes num_cpus.
    pub threads: Option<u32>,
    /// Optionally time out requests at a number of milliseconds. Note: checking timeouts
    /// costs CPU cycles; max performance will suffer. However, if you have an ill-behaving
    /// server that isn't connecting consistently and is hanging onto connections, this can
    /// improve the situation.
    pub connect_timeout: Option<u32>,
    /// Optionally time out read requests at a number of milliseconds. Note: checking timeouts
    /// costs CPU cycles; max performance will suffer. However, if you have an ill-behaving server
    /// that isn't sending EOF bytes or otherwise isn't dropping connections, this can be
    /// essential to maintaining a high(ish) throughput, at the cost of more CPU load.
    pub read_timeout: Option<u32>,
    /// Absolute number of requests to be made. Should split evenly across threads.
    pub limit: Option<u32>,
    /// Repeats the outgoing message multiple times per connection.
    pub repeat: u32,
    /// Fuzzing config toml file
    pub fuzz_path: Option<String>,
}

impl Config {
    pub fn new(target: SocketAddr) -> Config {
        let connections = 100; // todo: detect?
        Config {
            target,
            workers: connections,
            rate: None,
            limit: None,
            repeat: 1,
            duration: None,
            threads: None,
            read_timeout: None,
            connect_timeout: None,
            fuzz_path: None,
        }
    }

    /// Number of user-defined threads, or all the threads on the host.
    pub fn num_threads(&self) -> u32 {
        match self.threads {
            None => num_cpus::get() as u32,
            Some(n) => n,
        }
    }

    /// Number of connection loops each thread should maintain. Will never be less than the number
    /// of threads.
    pub fn connections_per_thread(&self) -> u32 {
        match self.workers / self.num_threads() as u32 {
            0 => 1,
            n => n,
        }
    }

    /// The amount a single connection should wait between loops in order to maintain the defined
    /// rate. Returns a default duration if there is no rate.
    pub fn connection_delay(&self) -> Duration {
        match self.rate {
            Some(rate) => {
                Duration::from_secs(1) / rate * self.connections_per_thread() * self.num_threads()
            }
            None => Duration::default(),
        }
    }

    /// The number of iterations each connection loop should perform before stopping. Doesn't play
    /// nice with limits that are not divisible by the number of threads and connections.
    pub fn limit_per_connection(&self) -> Option<u32> {
        match self.limit {
            None => None,
            Some(n) => Some(n / self.workers),
        }
    }
}

pub struct ConfigBuilder {
    config: Config,
}

// todo: Oof, this should be a macro
impl ConfigBuilder {
    pub fn new(target: SocketAddr) -> ConfigBuilder {
        ConfigBuilder {
            config: Config::new(target),
        }
    }

    pub fn build(self) -> Config {
        self.config
    }

    pub fn connections(mut self, connections: u32) -> ConfigBuilder {
        self.config.workers = connections;
        self
    }

    pub fn rate(mut self, rate: Option<u32>) -> ConfigBuilder {
        self.config.rate = rate;
        self
    }

    pub fn duration(mut self, duration: Option<Duration>) -> ConfigBuilder {
        self.config.duration = duration;
        self
    }

    pub fn threads(mut self, threads: Option<u32>) -> ConfigBuilder {
        self.config.threads = threads;
        self
    }

    pub fn connect_timeout(mut self, connect_timeout: Option<u32>) -> ConfigBuilder {
        self.config.connect_timeout = connect_timeout;
        self
    }

    pub fn read_timeout(mut self, read_timeout: Option<u32>) -> ConfigBuilder {
        self.config.read_timeout = read_timeout;
        self
    }

    pub fn limit(mut self, limit: Option<u32>) -> ConfigBuilder {
        self.config.limit = limit;
        self
    }

    pub fn repeat(mut self, repeat: u32) -> ConfigBuilder {
        self.config.repeat = repeat;
        self
    }

    pub fn fuzz_path(mut self, path: Option<String>) -> ConfigBuilder {
        self.config.fuzz_path = path;
        self
    }
}
