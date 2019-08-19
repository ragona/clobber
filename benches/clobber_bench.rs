#[macro_use]
extern crate criterion;

use criterion::black_box;
use criterion::Criterion;

use clobber::{tcp, util::test_server, Config, Message};

fn clobber(n: u32) {
    // todo: Figure out a way to benchmark this thing
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("clobber 250", |b| b.iter(|| clobber(black_box(250))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
