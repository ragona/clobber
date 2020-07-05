#![allow(dead_code)]

use async_std::{
    prelude::*,
    sync::{channel, Receiver, Sender},
};
use std::collections::VecDeque;

struct WorkerPool<In, Out, F> {
    /// How many workers we want
    num_workers: usize,
    /// How many workers we have
    cur_workers: usize,
    /// Outstanding tasks
    queue: VecDeque<In>,
    /// Where this pool sends results
    results: Sender<Out>,
    /// Where workers send results
    worker_send: Sender<Out>,
    /// Where we get results from workers
    worker_recv: Receiver<Out>,
    /// The async function that a worker performs
    task: fn(In, Sender<Out>) -> F,

    /// Internal event bus
    event_send: Sender<WorkerEvent>,
    event_recv: Receiver<WorkerEvent>,
}

#[derive(Debug, Copy, Clone)]
enum WorkerEvent {
    Done,
}

/// # WorkerPool
///
/// This is a bit of an odd implementation of a futures-oriented worker pool.
/// It's intended to be used with relatively long-running futures that all write out to the
/// same output channel of type `Out`.
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
impl<In, Out, F> WorkerPool<In, Out, F>
where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    pub fn new(task: fn(In, Sender<Out>) -> F, num_workers: usize, results: Sender<Out>) -> Self {
        let (worker_send, worker_recv) = channel(num_workers); // todo: I'm concerned about the size here
        let (event_send, event_recv) = channel(1024);

        Self {
            queue: VecDeque::with_capacity(num_workers),
            cur_workers: 0,
            num_workers,
            task,
            worker_recv,
            worker_send,
            results,
            event_recv,
            event_send,
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
        self.cur_workers
    }

    /// Target number of workers
    pub fn num_workers(&self) -> usize {
        self.num_workers
    }

    /// Whether the current number of workers is the target number of workers
    pub fn at_worker_capacity(&self) -> bool {
        self.cur_workers == self.num_workers
    }

    pub fn working(&self) -> bool {
        self.cur_workers > 0
    }

    /// Pops tasks from the queue if we have available worker capacity
    /// Sends out messages if any of our workers have delivered results
    ///
    /// todo: Bootleg stream/fut impl. Make it real.
    ///
    pub async fn work(&mut self) -> bool {
        // update state from our event bus
        while let Ok(event) = self.event_recv.try_recv() {
            match event {
                WorkerEvent::Done => {
                    self.cur_workers -= 1;
                }
            }
        }

        // get waiting results and send to consumer
        if let Ok(out) = self.worker_recv.try_recv() {
            self.results.send(out).await;
        }

        // add new workers
        while !self.queue.is_empty() && !self.at_worker_capacity() {
            self.cur_workers += 1;

            let task = self.queue.pop_front().unwrap(); // safe; we just checked empty
            let work_send = self.worker_send.clone();
            let event_send = self.event_send.clone();
            let fut = (self.task)(task, work_send);

            async_std::task::spawn(async move {
                fut.await;
                event_send.send(WorkerEvent::Done).await;
            });
        }

        self.working()
    }
}

async fn double_twice(x: usize, send: Sender<usize>) {
    send.send(x * 2).await;
    send.send(x * 2 * 2).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::sync::TryRecvError;
    use futures_await_test::async_test;

    #[async_test]
    async fn pool_test() {
        let (send, recv) = channel(100);
        let mut pool = WorkerPool::new(double_twice, 4, send);

        pool.push(1usize);
        pool.push(2);
        pool.push(3);
        pool.push(4);

        while pool.work().await {
            match recv.try_recv() {
                Ok(out) => {
                    dbg!(out);
                }
                Err(_) => {}
            }
        }
    }
}
