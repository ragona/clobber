use clobber::tuning::{graph_log, setup_logger};
use clobber::PidController;
use log::LevelFilter;
use std::path::Path;

const LOG_PATH: &str = "examples/.logs/simple.log";

fn main() {
    setup_logger(LevelFilter::Trace, Path::new(LOG_PATH)).unwrap();

    let mut controller = PidController::new((1.0, 1.0, 1.0));

    // Simulating a loadtest with a goal of 100 rps
    controller.update(100.0, 0.0);
    controller.update(100.0, 20.0);
    controller.update(100.0, 50.0);
    controller.update(100.0, 250.0);
    controller.update(100.0, 100.0);

    graph_log(Path::new(LOG_PATH)).unwrap();
}
