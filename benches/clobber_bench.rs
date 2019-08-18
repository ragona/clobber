#[macro_use]
extern crate criterion;

use criterion::black_box;
use criterion::Criterion;

use clobber::{tcp, util::test_server, Config, Message};

fn clobber(n: u32) {
    let (addr, _receiver) = test_server();
    let config = Config {
        target: addr,
        connections: 10,
        rate: None,
        limit: Some(n),
        duration: None,
        threads: Some(1),
        connect_timeout: None,
        read_timeout: None,
        repeat: false,
    };
    let message = Message::new(b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec());

    tcp::clobber(config, message).unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("clobber 250", |b| b.iter(|| clobber(black_box(250))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
