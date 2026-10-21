use thiserror::Error;
use tray_translate_core::CoreError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("core error: {0}")]
    Core(#[from] CoreError),

    #[error("invalid hotkey: {0}")]
    InvalidHotkey(String),

    #[error("invalid settings: {0}")]
    InvalidSettings(String),

    #[error("text is too long: {len} characters (max {max})")]
    TextTooLong { len: usize, max: usize },

    #[error("a download is already in progress: {0}")]
    DownloadInProgress(String),

    #[error("config directory not found for this platform")]
    ConfigDirNotFound,
}

pub type Result<T> = std::result::Result<T, AppError>;
