//! # PID Controller WorkerPool
//!
//! Attempting to drive throughput with
//!

use async_std::{prelude::*, sync::Sender, task};
use clobber::{tuning, PidController, WorkerPool};
use http_types::StatusCode;
use log::{info, warn, LevelFilter};
use std::time::{Duration, Instant};
use surf;
use tokio::runtime::Runtime;
use warp::Filter;

type JobResult = (StatusCode, Duration);

fn main() {
    start_logger(LevelFilter::Warn);
    start_test_server();

    task::block_on(async {
        let rps = 100;
        let ticks_per_second = 10;
        let tick_duration = Duration::from_millis(1000 / ticks_per_second as u64);
        let url = "http://localhost:8000/hello/server";

        let mut pool = WorkerPool::new(load_url_n_times, 1);
        let mut pid = PidController::new((0.8, 0.2, 0.2));

        loop {
            let start_time = Instant::now();

            // distribute work between workers
            let per_tick = rps / ticks_per_second;
            let per_worker = per_tick / pool.num_workers();
            for _ in 0..pool.num_workers() {
                pool.push((url, per_worker))
            }

            // perform the work
            while let Some(_) = pool.next().await {}

            // see how we did
            let duration = Instant::now().duration_since(start_time);
            let actual_rps = duration.as_secs_f32() * per_tick as f32;

            dbg!(actual_rps);

            // if we're ahead of the workload we can rest
            if duration < tick_duration {
                task::sleep(tick_duration).await;
            } else {
                println!("behind by {:.3}s", (duration - tick_duration).as_secs_f32())
            }
        }
    });
}

// this is a small-ish batch of work for one worker
async fn load_url_n_times(config: (&str, usize), send: Sender<JobResult>) {
    let (url, n) = config;
    for _ in 0..n {
        let start = Instant::now();
        let status = match surf::get(url).await {
            Ok(res) => res.status(),
            Err(err) => err.status(),
        };
        let diff = Instant::now().duration_since(start);

        send.send((status, diff)).await;
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
        .unwrap();
}
