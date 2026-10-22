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

    #[error("HTTP request error: {0}")]
    Http(#[from] Box<ureq::Error>),

    #[error("CTranslate2 engine error: {0}")]
    Ct2(#[from] anyhow::Error),

    #[error("Package metadata missing or invalid at {0}")]
    InvalidMetadata(PathBuf),

    #[error("Missing model file: {0}")]
    MissingModelFile(PathBuf),

    #[error("Invalid model package: {0}")]
    InvalidPackage(String),

    #[error("Standard data directory not found for this platform")]
    DataDirNotFound,

    #[error("Package not found in index for: {0} -> {1}")]
    PackageNotFound(String, String),

    #[error("No valid download link found for package")]
    NoDownloadLink,

    #[error("Unsupported language pair: {0} -> {1}")]
    UnsupportedLanguagePair(String, String),
}

impl From<ureq::Error> for CoreError {
    fn from(err: ureq::Error) -> Self {
        CoreError::Http(Box::new(err))
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
