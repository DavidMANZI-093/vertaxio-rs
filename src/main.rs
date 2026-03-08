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

use services::parser::Config;
use windows::Win32::System::Console::{FlushConsoleInputBuffer, GetStdHandle, STD_INPUT_HANDLE};

mod core;
mod services;
mod utils;

use services::input::{is_key_down, vk_to_string};

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

    let mut capture = core::capture::DXGICapture::new(cfg.monitor.hmonitor)?;

    let is_active = Arc::new(AtomicBool::new(false));
    let should_stop = Arc::new(AtomicBool::new(false));

    let should_stop_clone = should_stop.clone();
    let is_active_clone = is_active.clone();

    let target_frame_time = Duration::from_millis(1000 / cfg.fps as u64);

    let capture_width = capture.texture_desc.Width as i32;
    let capture_height = capture.texture_desc.Height as i32;

    let is_day_mode = Arc::new(AtomicBool::new(true));
    let is_day_mode_cv = Arc::clone(&is_day_mode);

    let cv_thread = std::thread::spawn(move || {
        let mut pipeline = core::vision::VisionPipeline::new(
            cfg.day_hsv_low,
            cfg.day_hsv_high,
            cfg.night_hsv_low,
            cfg.night_hsv_high,
            is_day_mode_cv,
        )
        .expect("Failed to initialize VisionPipeline");
        let mut debug_window = core::debug::DebugWindow::new(cfg.debug_mode, cfg.fps as usize);
        let mut frames = 0;
        let mut last_fps_print = Instant::now();

        loop {
            if should_stop_clone.load(Ordering::SeqCst) {
                break;
            }
            if is_active_clone.load(Ordering::SeqCst) {
                let frame_start = Instant::now();

                match capture.grab_frame(100) {
                    Ok(mut _buffer) => {
                        let targets = pipeline
                            .process_frame(capture_width, capture_height, &mut _buffer)
                            .unwrap_or_else(|e| {
                                utils::logger::error(&format!("Pipeline processing failed: {}", e));
                                Vec::new()
                            });

                        if let Err(e) =
                            debug_window.draw(capture_width, capture_height, &mut _buffer, &targets)
                        {
                            utils::logger::debug(&format!("Debug draw failed: {}", e));
                        }

                        if !targets.is_empty() {
                            utils::logger::debug(&format!(
                                "Found {} potential targets",
                                targets.len()
                            ));
                        }

                        frames += 1;
                    }
                    Err(services::errors::XError::Timeout) => {
                        debug_window.update();
                    }
                    Err(e) => {
                        debug_window.update();
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
                debug_window.update();
                std::thread::sleep(target_frame_time);
                last_fps_print = Instant::now();
                frames = 0;
            }
        }
    });

    utils::logger::info(&format!(
        "Hold {} to activate processing, press {} to exit.",
        vk_to_string(cfg.trigger_key),
        vk_to_string(cfg.exit_key)
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

    utils::logger::warn("Main loop exited. Flushing input...");

    // Flush the console input buffer so any keys pressed globally
    // don't get vomited onto the terminal prompt after we close.
    unsafe {
        if let Ok(stdin_handle) = GetStdHandle(STD_INPUT_HANDLE) {
            let _ = FlushConsoleInputBuffer(stdin_handle);
        }
    }

    Ok(())
}
