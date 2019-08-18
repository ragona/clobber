#[macro_use]
extern crate criterion;

use criterion::black_box;
use criterion::Criterion;

use clobber::{tcp, util::test_server, Config, ConfigBuilder, Message};

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
    c.bench_function("clobber_single_100", |b| {
        b.iter(|| clobber_single(black_box(100)))
    });
    c.bench_function("clobber_multi_100", |b| {
        b.iter(|| clobber_multi(black_box(100)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
