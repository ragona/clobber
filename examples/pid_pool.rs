//! # PID Controller WorkerPool
//!
//! Attempting to drive throughput with
//!

use async_std::{
    prelude::*,
    sync::{Receiver, Sender},
    task,
};
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
        let rps = 1000;
        let url = "http://localhost:8000/hello/server";

        let mut pool = WorkerPool::new(load_url_forver, 1);
        let mut pid = PidController::new((0.8, 0.0, 0.0));

        // note that we don't directly control the speed of this loop, we just assume the work takes time
        loop {
            let start_time = Instant::now();

            for _ in 0..pool.target_workers() {
                pool.push((url, 100))
            }

            // perform the work
            let mut total_work = 0;
            while let Some(_) = pool.next().await {
                total_work += 1;
            }

            // see how we did
            let duration = Instant::now().duration_since(start_time);
            let actual_rps = total_work as f32 / duration.as_secs_f32();

            // tell our controller about it
            pid.update(rps as f32, actual_rps);

            //
            dbg!(actual_rps, pid.output());
        }
    });
}

/// This is a single worker method that makes constant HTTP GET requests
/// until the Receiver channel gets a close method.
///
async fn load_url_forver(config: (&str, Receiver<()>), send: Sender<JobResult>) {
    let (url, n) = config;
    loop {
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
