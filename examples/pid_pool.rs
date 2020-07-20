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
use clobber::{Job, JobStatus, PidController, WorkerPool, WorkerPoolCommand};
use std::{
    cmp::Ordering::Equal,
    collections::{HashMap, VecDeque},
    fmt::{Debug, Formatter},
};

fn main() {
    start_logger(LevelFilter::Debug);
    start_test_server();

    task::block_on(async {
        let goal_rps = 10000f32;
        let url = "http://localhost:8000/hello/server";
        let tick_rate = Duration::from_secs_f32(0.1);
        let num_workers = 1;

        let (send, recv) = channel(num_workers);
        let mut pid = PidController::new((0.1, 0.5, 0.5));
        let mut pool = WorkerPool::new(load_url, send, num_workers);
        let mut overall_tracker = RequestTracker::new();
        let commands = pool.command_channel();

        // separate process to receive and analyze output from the worker queue
        task::spawn(async move {
            let mut tick_start = Instant::now();
            let mut next_tick = tick_start + tick_rate;
            let mut tick_tracker = RequestTracker::new();

            // loop through results on the output channel
            while let Ok(metric) = recv.recv().await {
                overall_tracker.add(metric);
                tick_tracker.add(metric);

                if Instant::now() > next_tick {
                    pid.update(goal_rps, tick_tracker.rps());
                    let mut new_worker_cnt = pid.output() * 0.01;
                    new_worker_cnt /= 1.0 / tick_rate.as_secs_f32();
                    commands
                        .send(WorkerPoolCommand::SetWorkerCount(new_worker_cnt.round() as usize));

                    debug!("{}, {}", new_worker_cnt.round(), tick_tracker.rps());

                    tick_start = Instant::now();
                    next_tick = tick_start + tick_rate;
                    tick_tracker = RequestTracker::new();
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
        for _ in 0..500 {
            pool.push((&url, None));
        }

        pool.work().await;
    });
}

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
    /// RequestTracker keeps track of the previous `size` requests
    count: usize,
    start: Instant,
}

impl RequestTracker {
    pub fn new() -> Self {
        Self { start: Instant::now(), count: 0 }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn add(&mut self, _metric: Metric) {
        self.count += 1;
    }

    pub fn rps(&self) -> f32 {
        self.count() as f32 / Instant::now().duration_since(self.start).as_secs_f32()
    }
}

impl Debug for RequestTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestTracker")
            .field("rps", &self.rps())
            .field("count", &self.count())
            .finish()
    }
}

/// This is a single worker method that makes constant HTTP GET requests
/// until the Receiver channel gets a close method.
async fn load_url(job: Job<(&str, Option<usize>), Metric>) -> JobStatus {
    let (url, mut count) = job.task;

    let mut get_status = || {
        if job.stop_requested() {
            return JobStatus::Stopped;
        }

        // no count means run until canceled
        if count.is_none() {
            return JobStatus::Running;
        }

        let remaining = count.unwrap(); // safe, just checked is_none

        if remaining == 0 {
            return JobStatus::Done;
        }

        count = Some(remaining - 1);

        JobStatus::Running
    };

    loop {
        match get_status() {
            JobStatus::Done => return JobStatus::Done,
            JobStatus::Stopped => return JobStatus::Stopped,
            JobStatus::Running => {}
        }

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
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}, {}, {}",
                record.target(),
                chrono::Local::now().format("%H:%M:%S.%3f"),
                message,
            ))
        })
        .chain(
            fern::Dispatch::new()
                .level(log_level)
                .filter(|metadata| metadata.target() == "clobber::pid")
                .chain(fern::log_file("examples/.logs/pid-tuning.log").unwrap()),
        )
        .chain(
            fern::Dispatch::new()
                .level(log_level)
                .filter(|metadata| metadata.target() == "pid_pool")
                .chain(fern::log_file("examples/.logs/results.log").unwrap())
                .chain(std::io::stdout()),
        )
        .apply()
        .expect("failed to start logger");
}

#[cfg(test)]
mod tests {
    use super::*;
    use clobber::tuning;
    use std::{error::Error, path::Path};

    #[test]
    fn graph_log() -> Result<(), Box<dyn Error>> {
        tuning::filter_log(Path::new("examples/.logs/pid-tuning.log"), "Proportional", "p.log")?;
        tuning::filter_log(Path::new("examples/.logs/pid-tuning.log"), "Integral", "i.log")?;
        tuning::filter_log(Path::new("examples/.logs/pid-tuning.log"), "Derivative", "d.log")?;
        tuning::filter_log(Path::new("examples/.logs/pid-tuning.log"), "PidController", "pid.log")?;
        tuning::filter_log(Path::new("examples/.logs/results.log"), "pid_pool", "rps.log")?;

        Ok(())
    }
}
