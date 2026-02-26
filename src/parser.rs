use serde::Deserialize;
use std::path::PathBuf;
use willhook::KeyboardKey as Key;

use crate::models::XError::{self, ConfigError};

#[derive(Debug)]
pub struct Config {
    pub exit_key: Key,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    pub exit_key: String,
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> Result<Config, XError> {
        let path = path.unwrap_or_else(|| PathBuf::from("lamine.yml"));

        let contents: String = std::fs::read_to_string(&path).unwrap();
        let raw_config: RawConfig = serde_yaml::from_str(&contents).unwrap();

        Self::validate(raw_config)
    }

    fn validate(cfg: RawConfig) -> Result<Config, XError> {
        let exit_key: Key = match cfg.exit_key.to_uppercase().as_str() {
            "ESCAPE" => Key::Escape,
            "Q" => Key::Q,
            "Z" => Key::Z,
            "X" => Key::X,
            "C" => Key::C,
            "K" => Key::K,
            _ => return Err(ConfigError(format!(
                "Invalid exit_key: '{}'. Allowed values are ('Escape', 'Q', 'Z', 'X', 'C', 'K').",
                cfg.exit_key
            ))
            .into()),
        };
        Ok(Config { exit_key })
    }
}
