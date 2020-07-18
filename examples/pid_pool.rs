//! # PID Controller WorkerPool
//!
//! Attempting to drive target HTTP request throughput via PID controller.
//!

use async_std::{sync::channel, task};
use http_types::StatusCode;
use log::{debug, warn, LevelFilter};
use std::time::{Duration, Instant};
use surf;
use tokio::runtime::Runtime;
use warp::Filter;

use crate::Distribution::Percentile;
use async_std::sync::Receiver;
use clobber::{Job, PidController, WorkerPool};
use std::{
    cmp::Ordering::Equal,
    collections::{HashMap, VecDeque},
    fmt::{Debug, Formatter},
};

#[derive(Debug, Copy, Clone)]
pub enum Distribution {
    Average,
    Percentile(f32),
}

#[derive(Debug, Copy, Clone)]
struct Metric {
    pub result: StatusCode,
    pub duration: Duration,
}

#[derive(Clone)]
struct RequestTracker {
    start: Instant,
    /// RequestTracker keeps track of the previous `size` requests
    size: usize,
    count: usize,
    responses: VecDeque<Metric>,
}

impl RequestTracker {
    pub fn new(size: usize) -> Self {
        Self { size, start: Instant::now(), count: 0, responses: VecDeque::with_capacity(size) }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn add(&mut self, metric: Metric) {
        if self.responses.len() >= self.size {
            self.responses.pop_front();
        }

        self.count += 1;
        self.responses.push_back(metric);
    }

    pub fn rps(&self) -> f32 {
        self.count() as f32 / Instant::now().duration_since(self.start).as_secs_f32()
    }

    pub fn latency(&self, distribution: Distribution, status: StatusCode) -> Duration {
        match distribution {
            Distribution::Percentile(n) => self.latency_percentile(n, status),
            Distribution::Average => self.latency_percentile(0.50, status),
        }
    }

    /// Returns the nth percentile (e.g. 0.5 for p50, 0.99 for p99, 1.0 for highest, 0.0 for lowest
    pub fn latency_percentile(&self, n: f32, status: StatusCode) -> Duration {
        if n < 0.0 || n > 1.0 {
            panic!("Expected a float between 0.0 and 1.0, got {}", n);
        }

        let mut latencies = self
            .responses
            .iter()
            .filter(|m| m.result == status)
            .map(|m| m.duration.as_secs_f32())
            .collect::<Vec<f32>>();

        if latencies.is_empty() {
            return Duration::default();
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Equal));

        let secs = if n == 0.0 {
            *latencies.first().unwrap()
        } else if n == 1.0 {
            *latencies.last().unwrap()
        } else {
            latencies[(latencies.len() as f32 * n).floor() as usize]
        };

        Duration::from_secs_f32(secs)
    }
}

impl Debug for RequestTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestTracker")
            .field("rps", &self.rps())
            .field("count", &self.count())
            .field("avg", &self.latency(Distribution::Average, StatusCode::Ok))
            .field("p0", &self.latency(Distribution::Percentile(0.0), StatusCode::Ok))
            .field("p50", &self.latency(Distribution::Percentile(0.5), StatusCode::Ok))
            .field("p99", &self.latency(Distribution::Percentile(0.99), StatusCode::Ok))
            .field("p100", &self.latency(Distribution::Percentile(1.0), StatusCode::Ok))
            .finish()
    }
}

fn main() {
    start_test_server();

    task::block_on(async {
        let goal_rps = 1000f32;
        let url = "http://localhost:8000/hello/server";
        let num_workers = 2;
        let tick_rate = Duration::from_secs_f32(0.1);

        let (send, recv) = channel(num_workers);
        let mut pid = PidController::new((0.8, 0.0, 0.0));
        let mut pool = WorkerPool::new(load_url, send, num_workers);
        let mut tracker = RequestTracker::new(goal_rps.floor() as usize);

        // separate process to receive and analyze output from the worker queue
        task::spawn(async move {
            let mut tick_start = Instant::now();
            let mut next_tick = tick_start + tick_rate;

            // loop through results on the output channel
            while let Ok(metric) = recv.recv().await {
                tracker.add(metric);

                if Instant::now() > next_tick {
                    tick_start = Instant::now();
                    next_tick = tick_start + tick_rate;

                    pid.update(goal_rps, tracker.rps());

                    println!(
                        "Tracker, {}, {}, {}, {}",
                        tracker.count(),
                        tracker.rps(),
                        tracker.latency_percentile(0.5, StatusCode::Ok).as_micros(),
                        tracker.latency_percentile(0.99, StatusCode::Ok).as_micros(),
                    );
                }
            }

            let tick_actual = Instant::now().duration_since(tick_start);
            if tick_actual < tick_rate {
                task::sleep(tick_rate - tick_actual).await;
            } else {
                warn!("falling behind; wanted tick rate of {:?} got {:?}", tick_rate, tick_actual);
            }
        });

        // Give each of our starting workers something to chew on. These last forever, so
        // in this case we just want one task per worker.
        for _ in 0..num_workers {
            pool.push((&url, None));
        }

        pool.work().await;
    });
}

/// This is a single worker method that makes constant HTTP GET requests
/// until the Receiver channel gets a close method.
async fn load_url(job: Job<(&str, Option<usize>), Metric>) {
    let (url, mut count) = job.task;

    let mut done = || {
        if job.stop_requested() {
            return true;
        }

        // no count means run until canceled
        if count.is_none() {
            return false;
        }

        let remaining = count.unwrap(); // safe, just checked is_none

        if remaining == 0 {
            return true;
        }

        count = Some(remaining - 1);

        false
    };

    while !done() {
        let start = Instant::now();
        let status = match surf::get(url).await {
            Ok(res) => res.status(),
            Err(err) => err.status(),
        };
        let diff = Instant::now().duration_since(start);

        job.results.send(Metric { result: status, duration: diff }).await;
    }
}

/// Spins off an OS thread for tokio/warp to start our test server.
/// It's a weird hack, but I didn't want to find out what happens if you put tokio inside
/// the async-std runtime, and this is a test. Don't do this in prod.
fn start_test_server() {
    std::thread::spawn(|| {
        let mut tokio_rt = Runtime::new().expect("Failed to start tokio runtime for test server");
        tokio_rt.block_on(async {
            // GET /hello/warp => 200 OK with body "Hello, warp!"
            let hello = warp::path!("hello" / String).map(|name| format!("Hello, {}!", name));
            warp::serve(hello).run(([127, 0, 0, 1], 8000)).await;
        });
    });
}

fn start_logger(log_level: LevelFilter) {
    fern::Dispatch::new()
        .format(|out, message, _| {
            out.finish(format_args!("{} {}", chrono::Local::now().format("%H:%M:%S,"), message))
        })
        .chain(std::io::stdout())
        .level(log_level)
        .apply()
        .expect("failed to start logger");
}
