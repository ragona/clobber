use clobber;
use clobber::Work;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender, TryRecvError};

pub fn criterion_benchmark(c: &mut Criterion) {
    let (send_work, recv_work) = bounded(100);
    let (send_result, recv_result) = unbounded();
    let (send_analysis, recv_analysis) = unbounded();

    clobber::work(recv_work, send_result);
    clobber::analyze(recv_result, send_analysis);

    c.bench_function("baseline", |b| {
        b.iter(|| {
            send_work.send(Work {}).unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
