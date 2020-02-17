use clobber::{Output, Task, Work};
use futures::prelude::*;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Receiver, Sender};

const NUM_WORKERS: usize = 8;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jobs = vec![Work::new(10), Work::new(10)];
    let (task_tx, task_rx) = channel(10);
    
    tokio::spawn(async move {
        generate_work(task_tx, jobs).await;
    });

    work(task_rx).await;

    Ok(())
}

async fn generate_work(mut task_tx: Sender<Task>, mut jobs: Vec<Work>) {
    loop {
        // todo: receive results, reorder jobs on priority (or make jobs a heap)

        if !jobs.is_empty() {
            let job = jobs.last_mut().unwrap();
            match job.next() {
                Some(task) => {
                    if let Err(_) = task_tx.send(task).await {
                        println!("we broke");
                        break;
                    }
                }
                None => {
                    jobs.pop().unwrap();
                }
            }
        } else {
            // todo: This is sort of a race condition where if new work doesn't
            // show up before all potential new work is found from outstanding 
            // requests we'll drop out too early.
            break;
        }
    }
}

async fn work(task_rx: Receiver<Task>) {
    task_rx
        .map(|task| {
            async move {
                // pretend to do work
                tokio::time::delay_for(Duration::from_millis(100)).await;

                // return some results
                Output { val: task.0 }
            }
        })
        .buffered(NUM_WORKERS)
        .for_each(|out| {
            async move {
                println!("completed {}", out.val)
            }
        })
        .await;
}
