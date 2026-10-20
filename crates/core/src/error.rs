use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("CTranslate2 engine error: {0}")]
    Ct2(#[from] anyhow::Error),

    #[error("Package metadata missing or invalid at {0}")]
    InvalidMetadata(PathBuf),

    #[error("Missing model file: {0}")]
    MissingModelFile(PathBuf),

    #[error("Standard data directory not found for this platform")]
    DataDirNotFound,

    #[error("Unsupported language pair: {0} -> {1}")]
    UnsupportedLanguagePair(String, String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
