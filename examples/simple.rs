use clobber::tuning::{graph_log, setup_logger};
use clobber::PidController;
use log::LevelFilter;

fn main() {
    setup_logger(LevelFilter::Trace, "examples/simple.log").unwrap();

    let mut controller = PidController::new((1.0, 1.0, 1.0));

    // Simulating a loadtest with a goal of 100 rps
    controller.update(100.0, 0.0);
    controller.update(100.0, 20.0);
    controller.update(100.0, 50.0);
    controller.update(100.0, 250.0);
    controller.update(100.0, 100.0);

    graph_log("examples/simple.log").unwrap();
}
