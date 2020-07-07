#![allow(dead_code)]

use async_std::{
    prelude::*,
    sync::{channel, Receiver, Sender},
    task,
};
use crossbeam_channel::{self, Receiver as CrossbeamReceiver, Sender as CrossbeamSender};
use std::collections::VecDeque;

/// # WorkerPool
///
/// This is a channels-oriented async worker pool.
/// It's intended to be used with relatively long-running futures that all write out to the
/// same output channel of type `Out`. The worker pool gathers all of that output in whatever
/// order it appears, and sends it to the output channel.
///
/// The number of workers in this implementation is intended as a best effort, not a fixed
/// count, with an eye towards being used in situations where we may want that number to go
/// up or down over time based on the environment conditions.
///
/// You could imagine that a system under load might decide to back off on the number of open
/// connections if it was experiencing resource contention, and conversely to add new workers
/// if the queue has grown and we aren't at our max worker count.
///
/// I'm not incredibly concerned about allocations in this model; `WorkerPool` is a higher level
/// abstraction than something like `crossbeam`. I built this for a client-side use case to
/// put a load test target under variable load from long-running workers that just sit and loop
/// TCP connections against a server.
///
pub struct WorkerPool<In, Out, F> {
    /// How many workers we want
    num_workers: usize,
    /// How many workers we have
    cur_workers: usize,
    /// Outstanding tasks
    queue: VecDeque<In>,
    /// Output channel
    output: Sender<Out>,
    /// In progress jobs represented as oneshot closers
    closers: VecDeque<Sender<()>>,
    /// The async function that a worker performs
    task: fn(Job<In, Out>) -> F,
    /// Cloneable return channel, given to workers
    worker_send: Sender<Out>,
    /// The channel we check for work from the workers
    worker_recv: Receiver<Out>,

    /// Internal event bus
    event_send: CrossbeamSender<WorkerEvent>,
    event_recv: CrossbeamReceiver<WorkerEvent>,
}

#[derive(Debug, Copy, Clone)]
enum WorkerEvent {
    Done,
}

// todo command channel

pub struct Job<In, Out> {
    task: In,
    close: Receiver<()>,
    results: Sender<Out>,
}

impl<In, Out> Job<In, Out> {
    pub fn new(task: In, close: Receiver<()>, results: Sender<Out>) -> Self {
        Self { task, close, results }
    }
}

impl<In, Out, F> WorkerPool<In, Out, F>
where
    In: Send + Sync + Unpin + 'static,
    Out: Send + Sync + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    pub fn new(task: fn(Job<In, Out>) -> F, output: Sender<Out>, num_workers: usize) -> Self {
        let (worker_send, worker_recv) = channel(num_workers);
        let (event_send, event_recv) = crossbeam_channel::unbounded();

        Self {
            queue: VecDeque::with_capacity(num_workers),
            closers: VecDeque::with_capacity(num_workers),
            cur_workers: 0,
            num_workers,
            worker_recv,
            worker_send,
            event_recv,
            event_send,
            output,
            task,
        }
    }

    /// Sets the target number of workers.
    /// Does not stop in-progress workers.
    pub fn set_num_workers(&mut self, n: usize) {
        self.num_workers = n;
    }

    /// Add a new task to the back of the queue
    pub fn push(&mut self, task: In) {
        self.queue.push_back(task);
    }

    /// Number of workers currently working
    pub fn cur_workers(&self) -> usize {
        self.closers.len()
    }

    /// Target number of workers
    pub fn target_workers(&self) -> usize {
        self.num_workers
    }

    /// Whether the current number of workers is the target number of workers
    pub fn at_target_worker_count(&self) -> bool {
        self.cur_workers == self.target_workers()
    }

    pub fn working(&self) -> bool {
        self.cur_workers > 0
    }

    pub fn try_next(&mut self) -> Option<Out> {
        match self.worker_recv.try_recv() {
            Ok(out) => Some(out),
            Err(_) => None,
        }
    }

    pub async fn start(&mut self) {
        task::block_on(async {
            loop {
                self.work().await;
                if !self.working() {
                    break;
                }
            }
        })
    }

    /// Pops tasks from the queue if we have available worker capacity
    /// Sends out messages if any of our workers have delivered results
    async fn work(&mut self) {
        // update state from our event bus
        while let Ok(event) = self.event_recv.try_recv() {
            match event {
                WorkerEvent::Done => {
                    self.cur_workers -= 1;
                }
            }
        }

        // get waiting results and send to consumer
        // this blocks on consumption, which gives us a nice property -- if a user only
        // wants a limited number of messages they can just read a limited number of times.
        while let Ok(out) = self.worker_recv.try_recv() {
            self.output.send(out).await;
        }

        if self.cur_workers() <= self.target_workers() {
            // add workers until we're full
            while !self.queue.is_empty() && !self.at_target_worker_count() {
                self.cur_workers += 1;
                let (close_send, close_recv) = channel(1); // oneshot closer
                self.closers.push_front(close_send);
                let task = self.queue.pop_front().unwrap(); // safe; we just checked empty
                let work_send = self.worker_send.clone();
                let event_send = self.event_send.clone();
                let job = Job::new(task, close_recv, work_send);
                let fut = (self.task)(job);

                async_std::task::spawn(async move {
                    fut.await;
                    event_send.send(WorkerEvent::Done).expect("failed to send internal event");
                });
            }
        } else {
            while !self.at_target_worker_count() {
                let closer = self.closers.pop_back().unwrap();
                closer.send(()).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;
    use futures_await_test::async_test;
    use std::time::Duration;

    /// Double the input until we receive a close message
    async fn double(job: Job<usize, usize>) {
        let mut x = job.task;
        loop {
            match job.close.try_recv() {
                Ok(_) => break,
                Err(_) => {}
            }

            x *= 2;

            job.results.send(x * 2).await;

            // easy there buddy
            task::sleep(Duration::from_millis(100)).await;
        }
    }

    #[async_test]
    async fn pool_test() {
        let num_workers = 2;
        let (send, recv) = channel(num_workers);
        let mut pool = WorkerPool::new(double, send, num_workers);

        pool.push(1);

        dbg!("ASDASDASD");
        task::spawn(async move {
            for _ in 0..100 {
                match recv.recv().await {
                    Ok(out) => {
                        dbg!(out);
                    }
                    Err(_) => {
                        println!("oh no");
                    }
                }
            }
        });

        pool.start().await;

        // todo make it ever stop
    }
}
