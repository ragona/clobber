use crossbeam_channel::{bounded, unbounded, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

pub struct Work {}
pub struct Result {}
pub struct Analysis {}

pub fn start(count: usize) {
    // todo: priority queue for outstanding work
    let (send_work, recv_work) = bounded(100); // num workers
    let (send_result, recv_result) = unbounded();
    let (send_analysis, recv_analysis) = unbounded();

    work(recv_work, send_result);
    analyze(recv_result, send_analysis);

    let mut i = 0;
    while i < count {
        send_work.send(Work {}).unwrap();
        i += 1;
    }
}

pub fn work(recv_work: Receiver<Work>, send_result: Sender<Result>) {
    thread::spawn(move || {
        loop {
            if let Ok(_) = recv_work.try_recv() {
                // do work
                send_result.send(Result {}).unwrap();
            }
        }
    });
}

pub fn analyze(recv_results: Receiver<Result>, send_analysis: Sender<Analysis>) {
    thread::spawn(move || loop {
        if let Ok(_) = recv_results.try_recv() {
            // do analysis
            send_analysis.send(Analysis {}).unwrap();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn foo() {
        start(100000);
        dbg!("done");
    }
}
