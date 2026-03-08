use serde::Deserialize;
use std::path::PathBuf;
use windows::Win32::UI::Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_C, VK_K, VK_M, VK_Q, VK_X, VK_Z};

use crate::core::monitor::{self, Monitor};
use crate::services::errors::XError::{self, ConfigError};

#[derive(Clone)]
pub struct Config {
    pub exit_key: VIRTUAL_KEY,
    pub trigger_key: VIRTUAL_KEY,
    pub mode_switch_key: VIRTUAL_KEY,
    pub fps: u8,
    pub debug_mode: bool,
    pub day_hsv_low: [i32; 3],
    pub day_hsv_high: [i32; 3],
    pub night_hsv_low: [i32; 3],
    pub night_hsv_high: [i32; 3],
    pub monitor: Monitor,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    exit_key: String,
    trigger_key: String,
    #[serde(default = "default_mode_switch_key")]
    mode_switch_key: String,
    fps: u8,
    #[serde(default)]
    debug_mode: bool,
    #[serde(default = "default_day_hsv_low")]
    day_hsv_low: [i32; 3],
    #[serde(default = "default_day_hsv_high")]
    day_hsv_high: [i32; 3],
    #[serde(default = "default_night_hsv_low")]
    night_hsv_low: [i32; 3],
    #[serde(default = "default_night_hsv_high")]
    night_hsv_high: [i32; 3],
}

fn default_mode_switch_key() -> String { "M".to_string() }

fn default_day_hsv_low() -> [i32; 3] { [0, 134, 78] }
fn default_day_hsv_high() -> [i32; 3] { [12, 255, 255] }
fn default_night_hsv_low() -> [i32; 3] { [0, 122, 78] }
fn default_night_hsv_high() -> [i32; 3] { [12, 255, 255] }

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
        let mode_switch_key: VIRTUAL_KEY = match cfg.mode_switch_key.to_uppercase().as_str() {
            "M" => VK_M,
            _ => {
                return Err(ConfigError(format!(
                    "Invalid mode_switch_key: '{}'. Allowed values are 'M'.",
                    cfg.mode_switch_key
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
        let debug_mode = cfg.debug_mode;

        // Enumerate and get the selected monitor
        let monitor = monitor::enumerate()?;

        let day_hsv_low = cfg.day_hsv_low;
        let day_hsv_high = cfg.day_hsv_high;
        let night_hsv_low = cfg.night_hsv_low;
        let night_hsv_high = cfg.night_hsv_high;

        Ok(Config {
            exit_key,
            trigger_key,
            mode_switch_key,
            fps,
            debug_mode,
            day_hsv_low,
            day_hsv_high,
            night_hsv_low,
            night_hsv_high,
            monitor,
        })
    }
}
