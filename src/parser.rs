use serde::Deserialize;
use std::path::PathBuf;
use willhook::KeyboardKey as Key;

use crate::models::XError::{self, ConfigError};
use crate::utils::{self, Monitor};

#[derive(Debug)]
pub struct Config {
    pub exit_key: Key,
    pub dect_off_key: Key,
    pub fps: u8,
    // pub monitor: Monitor,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    exit_key: String,
    dect_off_key: String,
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
        let exit_key: Key = match cfg.exit_key.to_uppercase().as_str() {
            "Q" => Key::Q,
            "Z" => Key::Z,
            "X" => Key::X,
            _ => {
                return Err(ConfigError(format!(
                    "Invalid exit_key: '{}'. Allowed values are 'Q', 'Z', and 'X'.",
                    cfg.exit_key
                ))
                .into());
            }
        };
        let dect_off_key: Key = match cfg.dect_off_key.to_uppercase().as_str() {
            "C" => Key::C,
            "K" => Key::K,
            _ => {
                return Err(ConfigError(format!(
                    "Invalid dect_off_key: '{}'. Allowed values are 'C', and 'K'.",
                    cfg.dect_off_key
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
        utils::enumerate();
        Ok(Config {
            exit_key,
            dect_off_key,
            fps,
        })
    }
}
