//! 常駐アプリのコア。
//!
//! 画面（ウィンドウ、トレイ）や OS 連携（ホットキー、クリップボード）に依存しない部分を持つ。
//! OS 連携は、このクレートが定義するトレイト（例: `ClipboardSource`）を実装して差し込む。

pub mod clipboard;
pub mod config;
pub mod controller;
pub mod error;
pub mod hotkey;
pub mod models;
pub mod paths;
pub mod service;

pub use clipboard::{ClipboardSource, ClipboardWatcher};
pub use config::Settings;
pub use controller::{Command, Controller, Event};
pub use error::{AppError, Result};
pub use hotkey::{Hotkey, Modifier};
pub use models::{
    installed_models, parse_pair_key, required_packages, DownloadState, DownloadTracker,
    InstalledModel,
};
pub use paths::{get_config_dir, get_settings_path, resolve_config_dir};
pub use service::{Clock, ServiceConfig, SystemClock, Translate, TranslationService};
