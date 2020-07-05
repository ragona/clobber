#![allow(dead_code)]

use async_std::prelude::*;
use async_std::sync::{channel, Receiver, Sender};
use async_std::task::JoinHandle;
use std::collections::VecDeque;

struct WorkerPool<In, Out, F>
where
    F: Future<Output = Out> + Send + 'static,
{
    num_workers: usize,
    working: Vec<F>,
    queue: VecDeque<In>,
    results: Sender<Out>,
    task: fn(In) -> F,
}

impl<In, Out, F> WorkerPool<In, Out, F>
where
    In: Send + 'static,
    Out: Send + 'static,
    F: Future<Output = Out> + Send + 'static,
{
    pub fn new(task: fn(In) -> F, num_workers: usize, results: Sender<Out>) -> Self {
        Self {
            queue: VecDeque::with_capacity(num_workers),
            working: vec![],
            num_workers,
            results,
            task,
        }
    }

    pub fn set_num_workers(&mut self, n: usize) {
        self.num_workers = n;
    }

    pub fn add(&mut self, task: In) {
        self.queue.push_back(task);
    }

    pub fn work(&mut self) {
        while !self.queue.is_empty() {
            self.supervise();
        }
    }

    /// Number of workers currently working
    pub fn cur_workers(&self) -> usize {
        self.working.len()
    }

    /// Number of workers requested
    pub fn num_workers(&self) -> usize {
        self.num_workers
    }

    /// Whether the current number of workers is the requested number of workers
    pub fn at_worker_capacity(&self) -> bool {
        self.cur_workers() == self.num_workers
    }

    fn supervise(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        if self.at_worker_capacity() {
            return;
        }

        while !self.queue.is_empty() && !self.at_worker_capacity() {
            let task = self.queue.pop_front().unwrap(); // safe because we just checked empty
            self.add_worker(task);
        }
    }

    fn add_worker(&mut self, task: In) {
        self.working.push((self.task)(task));
    }
}

async fn double(x: usize) -> usize {
    x * 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_await_test::async_test;

    #[async_test]
    async fn pool_test() {}
}
