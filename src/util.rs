use std::io::{stdin, Read};
use std::thread;
use std::time::Duration;

/// This is a bit of a weird way to do this, but I'm not sure what the better option is.
/// What this bit of code does is that it spins off a thread to listen to stdin, and then
/// sleeps for a moment to give that thread time to put something in the channel. It... works.
/// Surely there is a more idiomatic option though. ‾\_(ツ)_/‾
pub fn optional_stdin() -> Option<Vec<u8>> {
    let (sender, receiver) = std::sync::mpsc::channel();

    thread::spawn(move || {
        let sin = stdin();
        let mut bytes = vec![];

        sin.lock()
            .read_to_end(&mut bytes)
            .expect("Failed to read from stdin");

        sender.send(bytes).expect("Failed to send input");
    });

    thread::sleep(Duration::from_millis(1));

    match receiver.try_recv() {
        Ok(l) => Some(l),
        _ => None,
    }
}
