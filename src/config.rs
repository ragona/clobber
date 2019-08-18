use std::net::SocketAddr;
use std::time::Duration;

/// Settings for the load test
///
/// todo: Make write/read optional. (Enum?)
///
#[derive(Debug, Copy, Clone)]
pub struct Config {
    /// Socket address (ip and port) of the host we'll be calling
    pub target: SocketAddr,
    /// Connections is the a key knob to turn when tuning a performance test. Honestly
    /// 'connections' isn't the best name; it implies a certain persistance that
    pub connections: u32,
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
    /// essential to maintaing a high(ish) throughput, at the cost of more CPU load.
    pub read_timeout: Option<u32>,
    /// Absolute number of requests to be made. Should split evenly across threads.
    pub limit: Option<u32>,
    /// Will send write/read requests repeatedly while the connection is open. ("Keepalive")
    pub repeat: bool,
}

impl Config {
    // todo: builder pattern
    pub fn new(target: SocketAddr) -> Config {
        let connections = 100; // todo: detect?
        Config {
            target,
            connections,
            rate: None,
            limit: None,
            repeat: false,
            duration: None,
            threads: None,
            read_timeout: None,
            connect_timeout: None,
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
        match self.connections / self.num_threads() as u32 {
            0 => 1,
            n => n,
        }
    }

    /// The amount a single connection should wait between loops in order to maintain the defined
    /// rate. Returns a default loop if there is no rate.
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
            Some(n) => Some(n / self.connections),
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

    /// Consume the builder and return the inner object
    pub fn build(self) -> Config {
        self.config
    }

    pub fn connections(mut self, connections: u32) -> ConfigBuilder {
        self.config.connections = connections;
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

    pub fn repeat(mut self, repeat: bool) -> ConfigBuilder {
        self.config.repeat = repeat;
        self
    }
}
