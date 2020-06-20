use clobber::PidController;
use fern;
use log::LevelFilter;

pub fn setup_logger(log_level: LevelFilter) -> Result<(), Box<dyn std::error::Error>> {
    fern::Dispatch::new()
        .format(|out, message, _| {
            out.finish(format_args!(
                "{} {}",
                chrono::Local::now().format("[%H:%M:%S],"),
                message
            ))
        })
        .chain(std::io::stdout())
        .chain(fern::log_file("clobber.log")?)
        .level(log_level)
        .apply()?;

    Ok(())
}

fn main() {
    setup_logger(LevelFilter::Trace).unwrap();

    let mut controller = PidController::new((1.0, 1.0, 1.0));

    // Simulating a loadtest with a goal of 100 rps
    controller.update(100.0, 0.0);
    controller.update(100.0, 20.0);
    controller.update(100.0, 50.0);
    controller.update(100.0, 250.0);
    controller.update(100.0, 100.0);
}
