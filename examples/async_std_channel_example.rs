use async_std::{
    pin::Pin,
    prelude::*,
    stream, task,
    task::{Context, Poll},
};
use std::time::Duration;

struct SlowCounter {
    count: usize,
}

impl SlowCounter {
    pub fn new() -> Self {
        Self { count: Default::default() }
    }

    pub async fn count(&mut self) -> usize {
        task::sleep(Duration::from_secs_f32(0.1)).await;
        self.count += 1;
        self.count
    }
}

impl Stream for SlowCounter {
    type Item = usize;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(task::block_on(self.get_mut().count())))
    }
}

fn main() {
    task::block_on(async {
        let mut counter = SlowCounter::new();

        for _ in 0..10 {
            dbg!(counter.next().await);
        }
    });
}
