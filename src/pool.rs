use async_std::prelude::*;
use async_std::sync::{channel, Receiver, Sender};
use std::collections::VecDeque;

struct Worker<In, Out, F: Future<Output = Out>> {
    job: fn(In) -> F,
    incoming: Receiver<Sender<Out>>,
}

impl<In, Out, F> Worker<In, Out, F>
where
    F: Future<Output = Out>,
    In: Send + Sync,
    Out: Send + Sync,
{
    async fn work(self, input: In) -> Out {
        (self.job)(input).await
    }

    pub fn new(job: fn(In) -> F, incoming: Receiver<Sender<Out>>) -> Self {
        Self { job, incoming }
    }
}

struct WorkerPool<In, Out, F>
where
    F: Future<Output = Out>,
{
    incoming: VecDeque<In>,
    task: fn(In) -> F,
}

impl<In, Out, F> WorkerPool<In, Out, F>
where
    In: Send + Sync,
    Out: Send + Sync,
    F: Future<Output = Out>,
{
    pub fn new(task: fn(In) -> F, max_workers: usize) -> Self {
        Self {
            incoming: VecDeque::with_capacity(max_workers),
            task,
        }
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
    async fn single_worker() {
        let (send, recv) = channel(1);
        let doubler = Worker {
            job: double,
            incoming: recv,
        };

        assert_eq!(doubler.work(10).await, 20);
    }
}
