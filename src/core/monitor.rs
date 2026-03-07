use std::ptr;

use getch::Getch;
use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
use winapi::shared::windef::{HDC, HMONITOR, LPRECT};
use winapi::um::winuser::{EnumDisplayMonitors, GetMonitorInfoW, MONITORINFOEXW};

use crate::services::errors::XError;

use crate::utils::logger;

#[derive(Clone)]
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
            monitors.push(MonitorData { hmonitor, info });
        }
    }
    TRUE
}

/// Retrieves all connected monitors from the system.
pub fn get_all() -> Vec<Monitor> {
    let mut monitors_data: Vec<MonitorData> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(monitor_callback),
            &mut monitors_data as *mut _ as LPARAM,
        );
    }

    monitors_data
        .into_iter()
        .map(|m| {
            let r = m.info.rcMonitor;
            Monitor {
                width: (r.right - r.left) as u32,
                height: (r.bottom - r.top) as u32,
                hmonitor: m.hmonitor,
            }
        })
        .collect()
}

/// Enumerates monitors and prompts the user for selection if multiple are found.
pub fn enumerate() -> Result<Monitor, XError> {
    let monitors = get_all();

    if monitors.is_empty() {
        return Err(XError::ConfigError("No monitors found".into()));
    }

    if monitors.len() == 1 {
        let m = monitors.into_iter().next().unwrap();
        logger::info(&format!("Auto-selected only monitor: {}x{}", m.width, m.height));
        return Ok(m);
    }

    logger::info("Multiple monitors detected:");
    for (i, m) in monitors.iter().enumerate() {
        logger::info(&format!("   {}: {}x{}", i + 1, m.width, m.height));
    }
    logger::info(&format!("Select monitor [1-{}] or 0 to cancel:", monitors.len()));

    let ch = Getch::new()
        .getch()
        .map_err(|e| XError::ConfigError(format!("Read error: {}", e)))?;

    let choice_char = ch as char;
    let choice = choice_char.to_digit(10).unwrap_or(255) as u8;

    if choice == 0 {
        logger::warn("Cancelled monitor selection - exiting");
        std::process::exit(0);
    }

    if choice < 1 || choice > monitors.len() as u8 {
        logger::error(&format!("Invalid input '{}' - exiting", choice_char));
        std::process::exit(1);
    }

    let idx = (choice - 1) as usize;
    let selected = monitors.into_iter().nth(idx).unwrap();
    
    logger::info(&format!("Selected monitor {}: {}x{}", choice, selected.width, selected.height));
    Ok(selected)
}
