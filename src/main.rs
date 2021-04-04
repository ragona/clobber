use std::error::Error;
use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use hyper::Client;

const TEST_LENGTH_SEC: u64 = 5;
const NUM_WORKERS: u64 = 24;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let mut workers = FuturesUnordered::new();
    for _ in 0..NUM_WORKERS {
        workers.push(tokio::spawn(async { worker().await.unwrap() }))
    }

    let mut total_requests = 0;
    while let Some(count) = workers.next().await {
        total_requests += count?;
    }

    println!("{}/s", total_requests / TEST_LENGTH_SEC);
    Ok(())
}

async fn worker() -> Result<u64> {
    let mut count = 0;

    let client = Client::new();
    let end = Instant::now() + Duration::from_secs(TEST_LENGTH_SEC);

    while Instant::now() < end {
        // client.get("http://localhost:8080".parse().unwrap()).await?;
        count += 1;
    }

    Ok(count)
}
