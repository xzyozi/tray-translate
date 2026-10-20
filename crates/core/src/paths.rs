use crate::error::{CoreError, Result};
use std::env;
use std::path::PathBuf;

/// モデル・パッケージの保存先ディレクトリを取得する。
/// 環境変数 `TT_MODEL_DIR` が設定されている場合はそれを最優先とし、
/// 未設定の場合は OS 標準のデータディレクトリ配下の `packages` を返す。
pub fn get_model_dir() -> Result<PathBuf> {
    if let Ok(val) = env::var("TT_MODEL_DIR") {
        if !val.trim().is_empty() {
            return Ok(PathBuf::from(val));
        }
    }

    let data_dir = dirs::data_dir().ok_or(CoreError::DataDirNotFound)?;
    Ok(data_dir.join("tray-translate").join("packages"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model_dir_default() {
        let dir = get_model_dir().expect("data dir should resolve");
        assert!(dir.ends_with(PathBuf::from("tray-translate").join("packages")));
    }
}
