mod pid;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid() {
        let mut controller = pid::PidController::new((1.0, 1.0, 1.0));

        // Starting a loadtest with a goal of 100 rps
        controller.update(100.0, 0.0);
        controller.update(100.0, 20.0);
        controller.update(100.0, 50.0);
        controller.update(100.0, 250.0);
        controller.update(100.0, 100.0);
    }
}
