//! # Async WorkerPool example
//!
//! This is a mildly contrived example in which I have long running jobs that hit localhost
//! 1000x in a loop and push out a tuple with status code and how long the request took.
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
    start_test_server();

    // 1 worker
    run_batches(1, 1000, 1);
    run_batches(4, 1000, 1);
    run_batches(8, 1000, 1);

    println!();

    // 1 worker / 1 job
    run_batches(1, 1000, 1);
    run_batches(4, 1000, 4);
    run_batches(8, 1000, 8);

    println!();

    // 1 worker / 2 job
    run_batches(1, 1000, 1);
    run_batches(8, 1000, 4);
    run_batches(16, 1000, 8);

    println!();

    // 1 worker / 4 job
    run_batches(4, 1000, 1);
    run_batches(16, 1000, 4);
    run_batches(36, 1000, 8);
}

fn run_batches(num_batches: usize, batch_size: usize, num_workers: usize) {
    task::block_on(async {
        let mut pool = WorkerPool::new(load_url_n_times, num_workers);

        for _ in 0..num_batches {
            pool.push(("http://127.0.0.1:8000/hello/server", batch_size));
        }

        let start_time = Instant::now();
        let mut success_count = 0;
        while let Some((status, _)) = pool.next().await {
            if status == http_types::StatusCode::Ok {
                success_count += 1;
            }
        }

        let run_duration = Instant::now().duration_since(start_time).as_secs_f32();

        println!(
            "{}, {:.2}, {}, {}",
            success_count,
            run_duration,
            num_workers,
            success_count as f32 / run_duration,
        )
    })
}

async fn load_url_n_times(config: (&str, usize), send: Sender<Res>) {
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
