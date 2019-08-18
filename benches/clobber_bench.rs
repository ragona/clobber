#[macro_use]
extern crate criterion;

use criterion::Criterion;
use criterion::{black_box, Benchmark};

use clobber::{tcp, util::test_server, Config, ConfigBuilder, Message};
use std::time::Duration;

fn clobber_single(n: u32) {
    let (addr, _receiver) = test_server();
    let message = Message::new(b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec());
    let config = ConfigBuilder::new(addr)
        .connections(10)
        .limit(Some(n))
        .threads(Some(1))
        .build();

    tcp::clobber(config, message).unwrap();
}

fn clobber_multi(n: u32) {
    let (addr, _receiver) = test_server();
    let message = Message::new(b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec());
    let config = ConfigBuilder::new(addr)
        .connections(10)
        .limit(Some(n))
        .build();

    tcp::clobber(config, message).unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    //    c.bench_function("clobber_single_1", |b| {
    //        b.iter(|| clobber_single(black_box(10)))
    //    });
    //    c.bench_function("clobber_multi_1", |b| {
    //        b.iter(|| clobber_multi(black_box(10000)))
    //    });

    // todo: None of this works. We gotta break this down further.
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
