use thiserror::Error;

#[derive(Error, Debug)]

pub enum XError {
    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("System API error: {0}")]
    SystemError(String),

    #[error("Timeout")]
    Timeout,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
