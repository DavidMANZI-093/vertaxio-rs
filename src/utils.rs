use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::ptr;

use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
use winapi::shared::windef::{HDC, HMONITOR, LPRECT};
use winapi::um::wingdi::DISPLAY_DEVICEW;
use winapi::um::winuser::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, MONITORINFOEXW,
};

pub struct Monitor {
    pub name: String,
    pub width: u32,
    pub height: u32,
}
unsafe extern "system" fn monitor_callback(
    hmonitor: HMONITOR,
    _: HDC,
    _: LPRECT,
    data: LPARAM,
) -> BOOL {
    let monitors = &mut *(data as *mut Vec<MONITORINFOEXW>);
    let mut info: MONITORINFOEXW = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _) != 0 {
        monitors.push(info);
    }
    TRUE
}

pub fn enumerate() {
    let mut monitors_raw: Vec<MONITORINFOEXW> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(monitor_callback),
            &mut monitors_raw as *mut _ as LPARAM,
        );
    }

    for monitor in monitors_raw {
        unsafe {
            let mut device: DISPLAY_DEVICEW = std::mem::zeroed();
            device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

            if EnumDisplayDevicesW(monitor.szDevice.as_ptr(), 0, &mut device, 0) != 0 {
                let len = device
                    .DeviceString
                    .iter()
                    .position(|&i| i == 0)
                    .unwrap_or(128);
                let name = OsString::from_wide(&device.DeviceString[..len])
                    .to_string_lossy()
                    .into_owned();

                let rect = monitor.rcMonitor;
                println!(
                    "Name: {} | Resolution: {}x{}",
                    name,
                    rect.right - rect.left,
                    rect.bottom - rect.top
                );
            }
        }
    }
}
