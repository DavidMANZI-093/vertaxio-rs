use argh::FromArgs;
use std::{
    error,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VIRTUAL_KEY};

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

fn is_key_down(key: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(key.0 as i32) < 0 }
}

fn main() -> Result<(), Box<dyn error::Error>> {
    utils::logger::info("Starting application");

    let args: Args = argh::from_env();
    let cfg: Config = services::parser::Config::load(args.config)?;

    let mut capture = core::capture::DXGICapture::new(cfg.monitor.hmonitor)?;

    let is_active = Arc::new(AtomicBool::new(false));
    let should_stop = Arc::new(AtomicBool::new(false));

    let should_stop_clone = should_stop.clone();
    let is_active_clone = is_active.clone();

    let target_frame_time = Duration::from_millis(1000 / cfg.fps as u64);

    let cv_thread = std::thread::spawn(move || {
        let mut frames = 0;
        let mut last_fps_print = Instant::now();

        loop {
            if should_stop_clone.load(Ordering::SeqCst) {
                break;
            }
            if is_active_clone.load(Ordering::SeqCst) {
                let frame_start = Instant::now();

                match capture.grab_frame(100) {
                    Ok(_buffer) => {
                        // TODO: OpenCV logic will go here
                        frames += 1;
                    }
                    Err(services::errors::XError::Timeout) => {}
                    Err(e) => {
                        utils::logger::debug(&format!("Capture skipped: {}", e));
                    }
                }

                if last_fps_print.elapsed() >= Duration::from_secs(1) {
                    utils::logger::info(&format!("Capture FPS: {}", frames));
                    frames = 0;
                    last_fps_print = Instant::now();
                }

                let elapsed = frame_start.elapsed();
                if elapsed < target_frame_time {
                    std::thread::sleep(target_frame_time - elapsed);
                }
            } else {
                std::thread::sleep(target_frame_time);
                // Keep the FPS timer ticking even when paused so it doesn't dump a huge number upon resume
                last_fps_print = Instant::now();
                frames = 0;
            }
        }
    });

    utils::logger::info(&format!(
        "Hold {:?} to activate processing, press {:?} to exit.",
        cfg.trigger_key, cfg.exit_key
    ));

    let mut was_trigger_down = false;

    loop {
        if is_key_down(cfg.exit_key) {
            should_stop.store(true, Ordering::SeqCst);
            break;
        }

        let is_trigger_down = is_key_down(cfg.trigger_key);
        if is_trigger_down && !was_trigger_down {
            let was_active = is_active.swap(true, Ordering::SeqCst);
            if !was_active {
                utils::logger::info("Processing started");
            }
        } else if !is_trigger_down && was_trigger_down {
            let was_active = is_active.swap(false, Ordering::SeqCst);
            if was_active {
                utils::logger::info("Processing stopped");
            }
        }
        was_trigger_down = is_trigger_down;

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    cv_thread.join().unwrap();

    utils::logger::warn("Main loop exited");
    Ok(())
}
