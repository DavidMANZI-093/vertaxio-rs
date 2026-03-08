use thiserror::Error;

#[derive(Error, Debug)]

pub enum XError {
    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("System API error: {0}")]
    SystemError(String),

    #[error("Timeout")]
    Timeout,

    #[error("Vision processing error: {0}")]
    VisionError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
