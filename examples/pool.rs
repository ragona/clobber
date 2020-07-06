//! # Async WorkerPool example
//!
//! This is a mildly contrived example in which I have long running jobs that hit localhost
//! 100x in a loop and push out a tuple with status code and how long the request took.
//!
//!

use async_std::{prelude::*, sync::Sender, task};
use clobber::WorkerPool;
use http_types::StatusCode;
use std::time::{Duration, Instant};
use surf;
use tokio::runtime::Runtime;
use warp::Filter;

type Res = (StatusCode, Duration);

fn main() {
    task::block_on(async {
        let num_workers = 1;
        let num_jobs = 4;
        let mut pool = WorkerPool::new(load_a_hundred_times, num_workers);

        start_test_server().await;

        for _ in 0..num_jobs {
            pool.push("http://localhost:8000/");
        }

        let mut count = 0;
        while let Some(res) = pool.next().await {
            count += 1;
        }

        dbg!(count);
    })
}

async fn load_a_hundred_times(url: &str, send: Sender<Res>) {
    for _ in 0..100 {
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
async fn start_test_server() {
    std::thread::spawn(|| {
        let mut tokio_rt = Runtime::new().expect("Failed to start tokio runtime for test server");
        tokio_rt.block_on(async {
            // GET /hello/warp => 200 OK with body "Hello, warp!"
            let hello = warp::path!("hello" / String).map(|name| format!("Hello, {}!", name));
            warp::serve(hello).run(([127, 0, 0, 1], 8000)).await;
        });
    });
}
