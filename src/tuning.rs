//! # Tuning Utilities
//!
//! This is a series of helper methods that can be conditionally compiled
//! into `clobber` with the `tuning` flag in order to help debug and tune
//! your PID controllers.
//!
//! This is also used by the `clobber` test and example suite, and is a
//! good idea when you're first getting started in order to visually see
//! what you're doing.
//!
//! ## Dependencies
//!
//! Requires `gnuplot`. Haven't even attempted this on Windows, ymmv.
//!

use chrono;
use fern;
use log::LevelFilter;
use std::error::Error;
use std::fs;

/// Display a `gnuplot` chart based on an output log.
///
/// This function expects a path to the log output from `clobber` at
/// `debug` level. Usually the easiest way to do this is as follows:
/// ```
/// use clobber::PidController;
/// use clobber::tuning::{setup_logger, graph_log};
/// use log::LevelFilter;
///
/// setup_logger(LevelFilter::Debug, "simple.log").unwrap();
/// // ... do stuff!
/// graph_log("simple.log").expect("Failed to graph");
/// ```
pub fn graph_log(log_path: &str) -> Result<(), Box<dyn Error>> {
    let log = fs::read_to_string(log_path)?;
    let log_filter = |filter_string: &str| {
        log.lines()
            .filter(|s| s.contains(filter_string))
            .map(|s| s.into())
            .collect::<Vec<String>>()
    };

    let p_log = log_filter("Proportional");
    let i_log = log_filter("Integral");
    let d_log = log_filter("Derivative");
    let pid_log = log_filter("PidController");

    dbg!(pid_log);

    Ok(())
}

pub fn setup_logger(
    log_level: LevelFilter,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(filename)?;

    fern::Dispatch::new()
        .format(|out, message, _| {
            out.finish(format_args!(
                "{} {}",
                chrono::Local::now().format("[%H:%M:%S],"),
                message
            ))
        })
        .chain(std::io::stdout())
        .chain(log_file)
        .level(log_level)
        .apply()?;

    Ok(())
}
