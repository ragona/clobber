/// Tests a few different use cases.
/// todo: Automate test case generation with a random builder
use crate::server::echo_server;
use clobber::*;
use log::LevelFilter;
use std::time::Duration;

fn test_message() -> Message {
    Message::new(b"GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n".to_vec())
}

#[allow(dead_code)]
fn enable_trace_logs() {
    clobber::setup_logger(LevelFilter::Trace).unwrap();
}

#[test]
fn single_thread_limited_rate_and_total() -> std::io::Result<()> {
    let addr = echo_server()?;
    let config = ConfigBuilder::new(addr)
        .connections(1)
        .rate(Some(10))
        .limit(Some(20))
        .threads(Some(1))
        .build();

    tcp::clobber(config, test_message())?;

    Ok(())
}

#[test]
fn multi_thread_limited_rate_and_total() -> std::io::Result<()> {
    let addr = echo_server()?;
    let config = ConfigBuilder::new(addr)
        .rate(Some(10))
        .limit(Some(20))
        .connections(10)
        .threads(Some(2))
        .build();

    tcp::clobber(config, test_message())?;

    Ok(())
}

#[test]
fn rateless_with_duration() -> std::io::Result<()> {
    let addr = echo_server()?;
    let config = ConfigBuilder::new(addr)
        .connections(4)
        .threads(Some(2))
        .duration(Some(Duration::from_secs(1)))
        .build();

    tcp::clobber(config, test_message())?;

    Ok(())
}
