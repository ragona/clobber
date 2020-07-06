use async_std::{prelude::*, sync::Sender, task};
use clobber::WorkerPool;
use http_types::StatusCode;
use std::time::{Duration, Instant};
use surf;

type Out = (StatusCode, Duration);

async fn load_a_hundred_times(url: &str, send: Sender<Out>) {
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

fn main() {
    task::block_on(async {
        let mut pool = WorkerPool::new(load_a_hundred_times, 4);

        for _ in 0..8 {
            pool.push("http://localhost:8000");
        }

        while let Some(res) = pool.next().await {
            dbg!(res);
        }
    })
}
