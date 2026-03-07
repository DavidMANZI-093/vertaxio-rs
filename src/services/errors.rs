use thiserror::Error;

#[derive(Error, Debug)]

pub enum XError {
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
