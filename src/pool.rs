use async_std::prelude::*;
use crossbeam_channel::{unbounded, Receiver};

struct Worker<In, Out, F: Future<Output = Out>> {
    job: fn(In) -> F,
}

impl<In, Out, F> Worker<In, Out, F>
where
    F: Future<Output = Out>,
{
    pub async fn work(self, input: In) -> Out {
        (self.job)(input).await
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
        let doubler = Worker { job: double };
        assert_eq!(doubler.work(10).await, 20);
    }
}
