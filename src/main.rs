use std::error::Error;
use std::time::{Duration, Instant};

use hyper::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let end = Instant::now() + Duration::from_secs(5);
    let client = Client::new();
    let mut i = 0_usize;

    while Instant::now() < end {
        client.get("http://localhost:8080".parse().unwrap()).await?;
        i += 1;
    }

    println!("{}/s", i / 5);

    Ok(())
}
