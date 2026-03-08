use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VIRTUAL_KEY};

pub fn is_key_down(key: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(key.0 as i32) < 0 }
}

pub fn vk_to_string(vk: VIRTUAL_KEY) -> String {
    let code = vk.0;
    if (0x30..=0x39).contains(&code) || (0x41..=0x5A).contains(&code) {
        format!("'{}'", (code as u8) as char)
    } else {
        match code {
            0x10 => "SHIFT".to_string(),
            0x11 => "CTRL".to_string(),
            0x12 => "ALT".to_string(),
            0x01 => "Left Mouse".to_string(),
            0x02 => "Right Mouse".to_string(),
            0x04 => "Middle Mouse".to_string(),
            0x05 => "Mouse Button 4".to_string(),
            0x06 => "Mouse Button 5".to_string(),
            _ => format!("KeyCode({})", code),
        }
    }
}
