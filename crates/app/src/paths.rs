use crate::error::{AppError, Result};
use std::env;
use std::path::PathBuf;

/// 設定ファイル名
pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// 履歴ファイル名（設定ファイルと同じディレクトリに置く。Issue #89）
pub const HISTORY_FILE_NAME: &str = "history.json";

/// 設定ディレクトリを決定する（環境変数、OS 標準の順）。
/// 環境変数 `TT_CONFIG_DIR` が空でなければ最優先とし、
/// 未設定の場合は OS 標準の設定ディレクトリ配下の `tray-translate` を返す。
pub fn resolve_config_dir(
    env_value: Option<&str>,
    os_config_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(value) = env_value {
        if !value.trim().is_empty() {
            return Some(PathBuf::from(value));
        }
    }
    os_config_dir.map(|dir| dir.join("tray-translate"))
}

/// 現在の環境での設定ディレクトリを取得する
pub fn get_config_dir() -> Result<PathBuf> {
    let env_value = env::var("TT_CONFIG_DIR").ok();
    let dir = resolve_config_dir(env_value.as_deref(), dirs::config_dir());
    dir.ok_or(AppError::ConfigDirNotFound)
}

/// 設定ファイルのパスを取得する
pub fn get_settings_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join(SETTINGS_FILE_NAME))
}

/// 履歴ファイルのパスを取得する
pub fn get_history_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join(HISTORY_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_value_takes_priority() {
        let dir = resolve_config_dir(Some("custom-dir"), Some(PathBuf::from("os-dir")));
        assert_eq!(dir, Some(PathBuf::from("custom-dir")));
    }

    #[test]
    fn blank_env_value_falls_back_to_os_dir() {
        let dir = resolve_config_dir(Some("   "), Some(PathBuf::from("os-dir")));
        assert_eq!(dir, Some(PathBuf::from("os-dir").join("tray-translate")));
    }

    #[test]
    fn unset_env_uses_os_dir() {
        let dir = resolve_config_dir(None, Some(PathBuf::from("os-dir")));
        assert_eq!(dir, Some(PathBuf::from("os-dir").join("tray-translate")));
    }

    #[test]
    fn no_dir_available_returns_none() {
        assert_eq!(resolve_config_dir(None, None), None);
    }
}
