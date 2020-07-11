//! # PID Controller WorkerPool
//!
//! Attempting to drive target HTTP request throughput via PID controller.
//!

use async_std::{sync::channel, task};
use http_types::StatusCode;
use log::LevelFilter;
use std::time::{Duration, Instant};
use surf;
use tokio::runtime::Runtime;
use warp::Filter;

use clobber::{Job, PidController, WorkerPool};

type JobResult = (StatusCode, Duration);

fn main() {
    start_logger(LevelFilter::Warn);
    start_test_server();

    task::block_on(async {
        let goal_rps = 1000;
        let url = "http://localhost:8000/hello/server";
        let num_workers = 2;

        let (send, recv) = channel(num_workers);
        let mut pid = PidController::new((0.8, 0.0, 0.0));
        let mut pool = WorkerPool::new(load_url_forever, send, num_workers);

        // separate process to receive and analyze output from the worker queue
        task::spawn(async move {
            // every n duration show shit to the pid and see how our tps has been
            while let Ok(out) = recv.recv().await {
                dbg!(out);
            }
        });

        // Give each of our starting workers something to chew on. These last forever, so
        // in this case we just want one task per worker.
        for _ in 0..num_workers {
            pool.push(url);
        }

        pool.work().await;
    });
}

/// This is a single worker method that makes constant HTTP GET requests
/// until the Receiver channel gets a close method.
async fn load_url_forever(job: Job<&str, JobResult>) {
    loop {
        if job.stop_requested() {
            break;
        }

        let start = Instant::now();
        let status = match surf::get(job.task).await {
            Ok(res) => res.status(),
            Err(err) => err.status(),
        };
        let diff = Instant::now().duration_since(start);

        job.results.send((status, diff)).await;
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
