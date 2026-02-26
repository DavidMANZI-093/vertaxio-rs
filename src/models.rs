use thiserror::Error;

#[derive(Error, Debug)]

pub enum XError {
    #[error("Config error: {0}")]
    ConfigError(String),
}
