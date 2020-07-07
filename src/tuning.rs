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
use std::{error::Error, fs, fs::File, io::Write, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

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
pub fn graph_log(log_path: &Path) -> Result<()> {
    // load in the log of all events
    let log = fs::read_to_string(log_path)?;

    // split out to individual controller files to make gnuplot happier
    let log_filter = |filter_string: &str| {
        log.lines().filter(|s| s.contains(filter_string)).map(|s| s.into()).collect::<Vec<String>>()
    };

    // write out the filtered log files to the sub file
    let write_sublog = |lines: Vec<String>, path: &Path| -> Result<()> {
        let mut log_file = create_or_overwrite_file(path)?;
        for line in lines {
            // split into fields and drop the middle 'type' field (e.g. "Proportional")
            let fields = line.split(",").collect::<Vec<&str>>();
            let line = [fields[0], fields[2]].join(",");

            // Write reduced log to sublog file
            writeln!(&mut log_file, "{}", line)?;
        }

        Ok(())
    };

    let p_log = log_filter("Proportional");
    let i_log = log_filter("Integral");
    let d_log = log_filter("Derivative");
    let pid_log = log_filter("PidController");

    let log_directory = log_path.parent().expect("Failed to get log directory");

    write_sublog(p_log, log_directory.join("p.log").as_path())?;
    write_sublog(i_log, log_directory.join("i.log").as_path())?;
    write_sublog(d_log, log_directory.join("d.log").as_path())?;
    write_sublog(pid_log, log_directory.join("pid.log").as_path())?;

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
