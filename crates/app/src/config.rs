use crate::error::{AppError, Result};
use crate::hotkey::Hotkey;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 常駐アプリの設定。未指定の項目は既定値で補う（古い設定ファイルとの互換のため）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 翻訳を実行するホットキー（例: `Ctrl+Alt+T`）
    pub hotkey: String,
    /// 翻訳元の言語コード
    pub source_lang: String,
    /// 翻訳先の言語コード
    pub target_lang: String,
    /// 最後の翻訳からこの秒数が過ぎたらモデルを解放する。0 は解放しない
    pub idle_unload_secs: u64,
    /// クリップボードの変化を監視して自動翻訳する
    pub watch_clipboard: bool,
    /// 翻訳結果を自動でクリップボードにコピーする
    pub copy_result: bool,
    /// 翻訳対象とする最小文字数
    pub min_chars: usize,
    /// 翻訳対象とする最大文字数
    pub max_chars: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Alt+T".to_string(),
            source_lang: "en".to_string(),
            target_lang: "ja".to_string(),
            idle_unload_secs: 300,
            watch_clipboard: false,
            copy_result: true,
            min_chars: 1,
            max_chars: 5000,
        }
    }
}

impl Settings {
    /// 設定値の整合性を検査する
    pub fn validate(&self) -> Result<()> {
        Hotkey::parse(&self.hotkey)?;

        let validate_lang = |lang: &str, field: &str| -> Result<()> {
            let trimmed = lang.trim();
            if trimmed.is_empty() {
                return Err(AppError::InvalidSettings(format!(
                    "{field} must not be empty"
                )));
            }
            if trimmed.len() > 10
                || !trimmed
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                // '_' はペアキー (`<from>_<to>`) の区切りのため言語コードに使えない (ADR-005)
                return Err(AppError::InvalidSettings(format!(
                    "Invalid {field} format: '{trimmed}'. Expected standard language code without '_' (e.g. 'en', 'ja', 'zh-CN')"
                )));
            }
            Ok(())
        };

        validate_lang(&self.source_lang, "source_lang")?;
        validate_lang(&self.target_lang, "target_lang")?;

        if self.min_chars == 0 {
            return Err(AppError::InvalidSettings(
                "min_chars must be at least 1".to_string(),
            ));
        }
        if self.min_chars > self.max_chars {
            return Err(AppError::InvalidSettings(
                "min_chars must not exceed max_chars".to_string(),
            ));
        }
        Ok(())
    }

    /// 設定ファイルを読み込む。ファイルが無ければ既定値を返す。
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)?;
        let settings: Self = serde_json::from_str(&text)?;
        settings.validate()?;
        Ok(settings)
    }

    /// 設定ファイルを保存する。一時ファイルに書いてから置き換える。
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let json = serde_json::to_string_pretty(self)?;
        let temp_path = path.with_extension("json.tmp");
        if let Err(error) = fs::write(&temp_path, format!("{json}\n")) {
            let _ = fs::remove_file(&temp_path);
            return Err(error.into());
        }
        if let Err(error) = fs::rename(&temp_path, path) {
            // 置き換えに失敗した場合は一時ファイルを残さない
            let _ = fs::remove_file(&temp_path);
            return Err(error.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_valid() {
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::load(&dir.path().join("settings.json")).unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");

        let settings = Settings {
            hotkey: "Ctrl+Shift+F5".to_string(),
            source_lang: "ja".to_string(),
            target_lang: "en".to_string(),
            idle_unload_secs: 0,
            watch_clipboard: true,
            copy_result: false,
            min_chars: 2,
            max_chars: 100,
        };
        settings.save(&path).unwrap();

        assert_eq!(Settings::load(&path).unwrap(), settings);
        // 一時ファイルは残らない
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn save_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let first = Settings::default();
        first.save(&path).unwrap();

        let second = Settings {
            target_lang: "es".to_string(),
            ..Settings::default()
        };
        second.save(&path).unwrap();

        assert_eq!(Settings::load(&path).unwrap(), second);
    }

    #[test]
    fn partial_file_is_filled_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{ "target_lang": "en" }"#).unwrap();

        let settings = Settings::load(&path).unwrap();
        assert_eq!(settings.target_lang, "en");
        assert_eq!(settings.hotkey, Settings::default().hotkey);
        assert_eq!(settings.max_chars, Settings::default().max_chars);
        // 項目が追加される前の設定ファイルでも、自動コピーは既定値（有効）になる
        assert!(settings.copy_result);
    }

    #[test]
    fn invalid_json_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "not json").unwrap();
        let result = Settings::load(&path);
        assert!(matches!(result, Err(AppError::Json(_))));
    }

    #[test]
    fn invalid_values_are_rejected() {
        let bad_hotkey = Settings {
            hotkey: "T".to_string(),
            ..Settings::default()
        };
        let result = bad_hotkey.validate();
        assert!(matches!(result, Err(AppError::InvalidHotkey(_))));

        let empty_lang = Settings {
            target_lang: " ".to_string(),
            ..Settings::default()
        };
        let result = empty_lang.validate();
        assert!(matches!(result, Err(AppError::InvalidSettings(_))));

        let bad_lang_char = Settings {
            source_lang: "invalid/language!code".to_string(),
            ..Settings::default()
        };
        assert!(bad_lang_char.validate().is_err());

        let zero_min = Settings {
            min_chars: 0,
            ..Settings::default()
        };
        assert!(zero_min.validate().is_err());

        let reversed = Settings {
            min_chars: 10,
            max_chars: 5,
            ..Settings::default()
        };
        assert!(reversed.validate().is_err());
    }

    #[test]
    fn language_code_format_is_validated() {
        let with_lang = |source: &str, target: &str| Settings {
            source_lang: source.to_string(),
            target_lang: target.to_string(),
            ..Settings::default()
        };

        // 受理するコード
        assert!(with_lang("en", "ja").validate().is_ok());
        assert!(with_lang("zh-CN", "en").validate().is_ok());
        assert!(with_lang(" en ", "ja").validate().is_ok());

        // 拒否するコード（空・長すぎる・使えない文字。'_' はペアキーの区切りなので不可）
        assert!(with_lang("en", "zh_CN").validate().is_err());
        assert!(with_lang("", "ja").validate().is_err());
        assert!(with_lang("en", "abcdefghijk").validate().is_err());
        assert!(with_lang("e n", "ja").validate().is_err());
        assert!(with_lang("en", "ja/x").validate().is_err());
        assert!(with_lang("日本語", "en").validate().is_err());
    }

    #[test]
    fn save_failure_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        // 保存先をディレクトリにして rename を失敗させる
        let path = dir.path().join("settings.json");
        fs::create_dir(&path).unwrap();

        assert!(Settings::default().save(&path).is_err());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn load_rejects_invalid_values_in_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{ "hotkey": "T" }"#).unwrap();
        let result = Settings::load(&path);
        assert!(matches!(result, Err(AppError::InvalidHotkey(_))));
    }

    #[test]
    fn save_does_not_write_invalid_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let invalid = Settings {
            min_chars: 0,
            ..Settings::default()
        };
        assert!(invalid.save(&path).is_err());
        assert!(!path.exists());
    }
}
