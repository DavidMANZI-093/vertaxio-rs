use std::ptr;

use getch::Getch;
use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
use winapi::shared::windef::{HDC, HMONITOR, LPRECT};
use winapi::um::winuser::{EnumDisplayMonitors, GetMonitorInfoW, MONITORINFOEXW};

use crate::services::errors::XError;

pub struct Monitor {
    pub width: u32,
    pub height: u32,
    pub hmonitor: HMONITOR,
}

struct MonitorData {
    hmonitor: HMONITOR,
    info: MONITORINFOEXW,
}

unsafe extern "system" fn monitor_callback(
    hmonitor: HMONITOR,
    _: HDC,
    _: LPRECT,
    data: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(data as *mut Vec<MonitorData>) };
    let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    unsafe {
        if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _) != 0 {
            monitors.push(MonitorData {
                hmonitor: hmonitor,
                info: info,
            });
        }
    }
    TRUE
}

pub fn enumerate() -> Result<Monitor, XError> {
    let mut monitors: Vec<MonitorData> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(monitor_callback),
            &mut monitors as *mut _ as LPARAM,
        );
    }

    if monitors.len() == 1 {
        let monitor = monitors.into_iter().next().unwrap();
        let r = monitor.info.rcMonitor;

        return Ok(Monitor {
            width: (r.right - r.left) as u32,
            height: (r.bottom - r.top) as u32,
            hmonitor: monitor.hmonitor,
        });
    }

    println!("Multiple monitors detected:");
    for (i, m) in monitors.iter().enumerate() {
        let r = m.info.rcMonitor;

        println!(
            "{}: {}x{}",
            i + 1,
            (r.right - r.left) as u32,
            (r.bottom - r.top) as u32
        );
    }
    println!("Select monitor [1-{}] or 0 to cancel: ", monitors.len());

    let ch = Getch::new()
        .getch()
        .map_err(|e| XError::ConfigError(format!("Read error: {}", e)))?;

    let choice = (ch as char)
        .to_digit(10)
        .ok_or_else(|| {
            println!("Invalid input - exiting");
            std::process::exit(1);
        })
        .unwrap() as u8;

    if choice == 0 {
        println!("Cancelled - exiting");
        std::process::exit(0);
    }

    if choice < 1 || choice > monitors.len() as u8 {
        println!("Choice out of range - exiting");
        std::process::exit(1);
    }

    let idx = (choice - 1) as usize;

    let r = monitors[idx].info.rcMonitor;

    return Ok(Monitor {
        width: (r.right - r.left) as u32,
        height: (r.bottom - r.top) as u32,
        hmonitor: monitors[idx].hmonitor,
    });
}
