use async_std::task;
use clobber::tuning::{graph_log, setup_logger};
use clobber::PidController;
use log::LevelFilter;
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use surf;

const LOG_PATH: &str = "examples/.logs/simple.log";

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger(LevelFilter::Debug, Path::new(LOG_PATH)).unwrap();

    let mut controller = PidController::new((1.0, 1.0, 1.0));

    // Simulating a loadtest with a goal of 100 rps
    controller.update(100.0, 0.0);

    graph_log(Path::new(LOG_PATH)).unwrap();

    task::block_on(async {
        loop {
            println!("yoooooo");
            task::sleep(Duration::from_secs(1)).await;
        }
    })
}
