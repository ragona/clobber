use clobber::{Output, Work};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::delay_for;

const NUM_WORKERS: usize = 100;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut work_tx, work_rx) = mpsc::channel(NUM_WORKERS);
    let (output_tx, mut output_rx) = mpsc::channel(NUM_WORKERS);

    let mut jobs = vec![Work::new(10), Work::new(10)];

    tokio::spawn(workers(work_rx, output_tx));

    loop {
        match output_rx.try_recv() {
            Ok(out) => {
                dbg!(out);
            }
            _ => {}
        }

        while !jobs.is_empty() {
            println!("sent");
            work_tx.send(jobs.pop().unwrap()).await?;
        }
    }
}

pub async fn workers(mut work_rx: mpsc::Receiver<Work>, output_tx: mpsc::Sender<Output>) {
    loop {
        if let Some(job) = work_rx.recv().await {
            let mut output_tx = output_tx.clone();
            tokio::spawn(async move {
                for task in job {
                    delay_for(Duration::from_millis(10)).await;
                    output_tx.send(Output {}).await.unwrap();
                }
            });
        }
    }
}
