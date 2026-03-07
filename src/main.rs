use argh::FromArgs;
use std::{
    error,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use willhook::{keyboard_hook, InputEvent, IsSystemKeyPress::Normal, KeyPress};

use services::parser::Config;

mod core;
mod services;
mod utils;

#[derive(FromArgs)]
/// Vertaxio, aim as good as masterchief, bruv...
struct Args {
    /// path to config file. (default lamine.yml)
    #[argh(option, short = 'c')]
    config: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn error::Error>> {
    utils::logger::info("Starting application");

    let args: Args = argh::from_env();
    let cfg: Config = services::parser::Config::load(args.config)?;

    let rx = keyboard_hook().unwrap();

    let is_processing = Arc::new(AtomicBool::new(false));
    let should_stop = Arc::new(AtomicBool::new(false));

    let should_stop_clone = should_stop.clone();
    let is_processing_clone = is_processing.clone();
    let cv_thread = std::thread::spawn(move || {
        loop {
            if should_stop_clone.load(Ordering::SeqCst) {
                break;
            }
            if is_processing_clone.load(Ordering::SeqCst) {
                // TODO: processing
            }
            std::thread::sleep(Duration::from_millis(1000 / cfg.fps as u64));
        }
    });

    loop {
        match rx.try_recv() {
            Ok(InputEvent::Keyboard(ke)) if ke.pressed == KeyPress::Down(Normal) => {
                if ke.key == Some(cfg.exit_key) {
                    should_stop.store(true, Ordering::SeqCst);
                    break;
                }
            }
            Ok(_) => {}
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    cv_thread.join().unwrap();

    utils::logger::warn("Main loop exited");
    Ok(())
}
