//! # Tuning Utilities
//!
//! This is a series of helper methods that can be conditionally compiled
//! into `clobber` with the `tuning` flag in order to help debug and tune
//! your PID controllers.
//!
//! ```toml
//! [dependencies.clobber]
//! version = "0.1.0"
//! features = ["tuning"]
//! ```
//!
//! This is also used by the `clobber` test and example suite, and is a
//! good idea when you're first getting started in order to visually see
//! what you're doing.
//!
//! ## Dependencies
//!
//! Requires `gnuplot`. Haven't even attempted this on anything except
//! linux. Ymmv.
//!

use chrono;
use fern;
use log::LevelFilter;
use std::{error::Error, fs, fs::File, io::Write, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Display a `gnuplot` chart based on an output log.
/// This function expects a path to the log output from `clobber` at
/// `debug` level. Usually the easiest way to do this is as follows:
/// ```
/// use clobber::PidController;
/// use clobber::tuning::{setup_logger, filter_log};
/// use log::LevelFilter;
///
/// setup_logger(LevelFilter::Debug, "simple.log".into()).unwrap();
/// // ... do stuff!
/// filter_log("simple.log".into(), "clobber::pid", "").expect("Failed to graph");
/// ```
///
/// Check out the format below for what it's expecting if you want add your own lines.
/// We only care about time and value. (i.e. 1, 3)
/// ```txt
/// [src/tuning.rs:51] &fields = [
///     0 "clobber::pid",
///     1 " 11:50:19.900",
///     2 " PidController",
///     3  " -464.55838",
/// ]
/// ```
pub fn filter_log(full_log: &Path, filter_string: &str, new_log_name: &str) -> Result<()> {
    // load in the log of all events
    let log = fs::read_to_string(full_log)?;

    // split out to individual controller files to make gnuplot happier
    let filtered_log = log
        .lines()
        .filter(|s| s.contains(filter_string))
        .map(|s| s.into())
        .collect::<Vec<String>>();

    // write out the filtered log files to the same folder the parent is in
    let new_log_path = full_log.parent().unwrap().join(Path::new(new_log_name));
    let mut sub_log_file = create_or_overwrite_file(new_log_path.as_path())?;
    for line in filtered_log {
        let fields = line.split(",").map(|s| s.trim()).collect::<Vec<&str>>();
        // we only care about time and value. (i.e. 1, 3)
        // &fields = [
        //     0 "clobber::pid",
        //     1 " 11:50:19.900",
        //     2 " PidController",
        //     3  " -464.55838",
        // ]
        let line = [fields[1], fields[3]].join(",");

        writeln!(&mut sub_log_file, "{}", line)?;
    }

    Ok(())
}

pub fn setup_logger(log_level: LevelFilter, path: &Path) -> Result<()> {
    let log_file = create_or_overwrite_file(path)?;

    fern::Dispatch::new()
        .format(|out, message, _| {
            out.finish(format_args!("{} {}", chrono::Local::now().format("%H:%M:%S,"), message))
        })
        .chain(std::io::stdout())
        .chain(log_file)
        .level(log_level)
        .apply()?;

    Ok(())
}

fn create_or_overwrite_file(path: &Path) -> Result<File> {
    // attempt to delete
    std::fs::remove_file(path).ok();

    Ok(std::fs::OpenOptions::new().write(true).create(true).open(path)?)
}
