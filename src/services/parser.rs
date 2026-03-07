use serde::Deserialize;
use std::path::PathBuf;
use windows::Win32::UI::Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_C, VK_K, VK_Q, VK_X, VK_Z};

use crate::core::monitor::{self, Monitor};
use crate::services::errors::XError::{self, ConfigError};

#[derive(Clone)]
pub struct Config {
    pub exit_key: VIRTUAL_KEY,
    pub trigger_key: VIRTUAL_KEY,
    pub fps: u8,
    pub monitor: Monitor,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    exit_key: String,
    trigger_key: String,
    fps: u8,
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Config, XError> {
        let path = path.unwrap_or_else(|| PathBuf::from("lamine.yml"));

        let contents: String = std::fs::read_to_string(&path)?;
        let raw_config: RawConfig = serde_yaml::from_str(&contents).unwrap();

        Self::validate(raw_config)
    }

    fn validate(cfg: RawConfig) -> Result<Config, XError> {
        let exit_key: VIRTUAL_KEY = match cfg.exit_key.to_uppercase().as_str() {
            "Q" => VK_Q,
            "Z" => VK_Z,
            "X" => VK_X,
            _ => {
                return Err(ConfigError(format!(
                    "Invalid exit_key: '{}'. Allowed values are 'Q', 'Z', and 'X'.",
                    cfg.exit_key
                ))
                .into());
            }
        };
        let trigger_key: VIRTUAL_KEY = match cfg.trigger_key.to_uppercase().as_str() {
            "C" => VK_C,
            "K" => VK_K,
            _ => {
                return Err(ConfigError(format!(
                    "Invalid trigger_key: '{}'. Allowed values are 'C', and 'K'.",
                    cfg.trigger_key
                ))
                .into());
            }
        };
        let fps: u8 = cfg.fps;
        if !(30..=120).contains(&fps) {
            return Err(ConfigError(format!(
                "Invalid fps: '{}'. Must be between 30 and 120.",
                cfg.fps
            ))
            .into());
        }
        
        // Enumerate and get the selected monitor
        let monitor = monitor::enumerate()?;
        
        Ok(Config {
            exit_key,
            trigger_key,
            fps,
            monitor,
        })
    }
}
