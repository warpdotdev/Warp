use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("listener consumer disconnected")]
    Disconnected,
}

pub type Result<T> = std::result::Result<T, Error>;
