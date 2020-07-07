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
/// abstraction than something like `crossbeam`. I built this for a client-side CLI use case to
/// put a load test target under variable load from long-running workers that just sit and loop
/// TCP connections against a server.
///
pub struct WorkerPool<In, Out, F> {
    /// How many workers we want
    num_workers: usize,
    /// How many workers we actually have
    cur_workers: usize,
    /// Outstanding tasks
    queue: VecDeque<In>,
    /// Output channel
    output: Sender<Out>,
    /// The async function that a worker performs
    task: fn(Job<In, Out>) -> F,
    /// Cloneable return channel sender, given to workers
    worker_send: Sender<Out>,
    /// The channel receiver we check for work from the workers
    worker_recv: Receiver<Out>,
    /// The channel sender we use to stop workers
    close_send: Sender<()>,
    /// Cloneable close channel receiver, given to workers
    close_recv: Receiver<()>,

    /// Unbounded internal event and command bus, processed every tick.
    event_send: CrossbeamSender<WorkerPoolEvent>,
    event_recv: CrossbeamReceiver<WorkerPoolEvent>,
}

#[derive(Debug, Copy, Clone)]
enum WorkerPoolEvent {
    WorkerDone,
    Command(WorkerPoolCommand),
}

#[derive(Debug, Copy, Clone)]
enum WorkerPoolCommand {
    Stop,
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
        let (close_send, close_recv) = channel(num_workers);
        let (event_send, event_recv) = crossbeam_channel::unbounded();

        Self {
            queue: VecDeque::with_capacity(num_workers),
            cur_workers: 0,
            num_workers,
            worker_send,
            worker_recv,
            close_send,
            event_recv,
            event_send,
            close_recv,
            output,
            task,
        }
    }

    /// Number of workers currently working
    /// This is the number of workers we haven't tried to stop yet plus the workers that haven't
    /// noticed they were told to stop.
    pub fn cur_workers(&self) -> usize {
        self.cur_workers
    }

    /// Target number of workers
    pub fn target_workers(&self) -> usize {
        self.num_workers
    }

    /// Whether the current number of workers is the target number of workers
    /// Adjusted for the number of workers that we have TOLD to stop but have
    /// not actually gotten around to stopping yet.
    pub fn at_target_worker_count(&self) -> bool {
        self.cur_workers() == self.target_workers()
    }

    pub fn working(&self) -> bool {
        self.cur_workers() > 0
    }

    /// Sets the target number of workers.
    /// Does not stop in-progress workers.
    pub fn set_target_workers(&mut self, n: usize) {
        self.num_workers = n;
    }

    /// Add a new task to the back of the queue
    pub fn push(&mut self, task: In) {
        self.queue.push_back(task);
    }

    /// Attempts to grab any immediately available results from the workers
    /// todo: Eh, I'm not sure this is a good API.
    pub fn try_next(&mut self) -> Option<Out> {
        match self.worker_recv.try_recv() {
            Ok(out) => Some(out),
            Err(_) => None,
        }
    }

    pub async fn start(&mut self) {
        task::block_on(async {
            loop {
                // get waiting results and send to consumer
                self.flush_output().await;

                // update state from our event bus
                if !self.event_loop() {
                    break;
                }

                // Get us to our target number of workers
                self.balance_workers().await;

                if !self.working() {
                    break;
                }
            }
        })
    }

    /// Returns whether or not to continue execution.
    fn event_loop(&mut self) -> bool {
        while let Ok(event) = self.event_recv.try_recv() {
            match event {
                WorkerPoolEvent::WorkerDone => {
                    // This means a worker actually stopped; either on its own or from a stop cmd.
                    self.cur_workers -= 1;
                }
                WorkerPoolEvent::Command(command) => match command {
                    WorkerPoolCommand::Stop => {
                        return false;
                    }
                },
            }
        }

        true
    }

    /// Flush all outstanding work results to the output channel.
    ///
    /// This blocks on consumption, which gives us a nice property -- if a user only
    /// wants a limited number of messages they can just read a limited number of times.
    /// This ends up only updating the async state machine that number of times, which
    /// is the "lazy" property of async we wanted to achieve.
    async fn flush_output(&mut self) {
        while let Ok(out) = self.worker_recv.try_recv() {
            self.output.send(out).await;
        }
    }

    /// Starts a new worker if there is work to do
    fn start_worker(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        let task = self.queue.pop_front().unwrap();
        let work_send = self.worker_send.clone();
        let event_send = self.event_send.clone();
        let close_recv = self.close_recv.clone();
        let job = Job::new(task, close_recv, work_send);
        let fut = (self.task)(job);

        // If a worker stops on its own without us telling it to stop then we want to know about
        // it so that we can spin up a replacement. This is done through an unbounded crossbeam
        // channnel that is processed every tick to update state.
        async_std::task::spawn(async move {
            fut.await;
            event_send.send(WorkerPoolEvent::WorkerDone).expect("failed to send WorkerEvent::Done");
        });

        self.cur_workers += 1;
    }

    /// Find a listening worker and tell it to stop.
    /// Doesn't forcibly kill in-progress tasks.
    async fn send_stop_work_message(&mut self) {
        self.close_send.send(()).await;
    }

    /// Pops tasks from the queue if we have available worker capacity
    /// Sends out messages if any of our workers have delivered results
    async fn balance_workers(&mut self) {
        if self.cur_workers() <= self.target_workers() {
            // add workers until we're full
            while !self.queue.is_empty() && !self.at_target_worker_count() {
                self.start_worker();
            }
        } else {
            while !self.at_target_worker_count() {
                self.send_stop_work_message().await;
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

    /// Double the input some number of times or until we receive a close message
    async fn double(job: Job<(usize, usize), usize>) {
        let (mut i, n) = job.task;
        for _ in 0..n {
            match job.close.try_recv() {
                Ok(_) => break,
                Err(_) => {}
            }

            i *= 2;

            job.results.send(i).await;

            // pretend this is hard
            task::sleep(Duration::from_millis(100)).await;
        }
    }

    #[async_test]
    async fn pool_test() {
        let num_workers = 2;
        let (send, recv) = channel(num_workers);
        let mut pool = WorkerPool::new(double, send, num_workers);

        pool.push((1, 10));
        pool.push((3, 10));
        pool.push((6, 2));

        // separate process to receive and analyze output from the worker queue
        task::spawn(async move {
            while let Ok(out) = recv.recv().await {
                dbg!(out);
            }
        });

        pool.start().await;

        // todo make it ever stop
    }
}
