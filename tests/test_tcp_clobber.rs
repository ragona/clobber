/// Tests a few different use cases.
/// todo: Automate test case generation with a random builder
use clobber::*;
use log::LevelFilter;
use std::net::SocketAddr;
use std::time::Duration;

fn test_message() -> Vec<u8> {
    b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec()
}

#[allow(dead_code)]
fn enable_trace_logs() {
    clobber::setup_logger(LevelFilter::Trace).unwrap();
}

fn echo_server() -> SocketAddr {
    "0.0.0.0:8000".parse().unwrap()
}

#[tokio::test]
async fn single_thread_limited_rate_and_total() -> std::io::Result<()> {
    let addr = echo_server();
    let config = ConfigBuilder::new(addr)
        .connections(1)
        .rate(Some(10))
        .limit(Some(20))
        .threads(Some(1))
        .build();

    clobber::go(config, test_message()).await;

    Ok(())
}

#[tokio::test]
async fn multi_thread_limited_rate_and_total() -> std::io::Result<()> {
    let addr = echo_server();
    let config = ConfigBuilder::new(addr)
        .rate(Some(10))
        .limit(Some(20))
        .connections(10)
        .threads(Some(2))
        .build();

    clobber::go(config, test_message()).await;

    Ok(())
}

#[tokio::test]
async fn rateless_with_duration() -> std::io::Result<()> {
    let addr = echo_server();
    let config = ConfigBuilder::new(addr)
        .connections(4)
        .threads(Some(2))
        .duration(Some(Duration::from_secs(1)))
        .build();

    clobber::go(config, test_message()).await;

    Ok(())
}

#[tokio::test]
async fn with_fuzz_config() -> std::io::Result<()> {
    let addr = echo_server();
    let config = ConfigBuilder::new(addr)
        .connections(1)
        .threads(Some(1))
        .limit(Some(10))
        .fuzz_path(Some(String::from("tests/fuzz_config.toml")))
        .build();

    clobber::go(config, b"foo".to_vec()).await;

    Ok(())
}
