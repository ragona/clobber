use criterion::{black_box, criterion_group, criterion_main, Criterion};
use crossbeam_channel::unbounded;

fn clobber(_: u32) {
    // todo: Figure out a way to benchmark this thing
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("clobber 100", |b| b.iter(|| clobber(black_box(100))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
