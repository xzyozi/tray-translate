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
    /// 入力後に自動翻訳する（既定: false / ADR-006）
    pub auto_translate: bool,
    /// 入力後の自動翻訳デバウンス待機時間（ミリ秒、既定: 800ms / ADR-006）
    pub auto_translate_delay_ms: u64,
    /// 「×」ボタンでタスクトレイに常駐するか。false なら「×」でプロセスを終了する。
    /// 既定値は OS で異なる（[`default_use_tray`] / ADR-007）。
    pub use_tray: bool,
    /// 履歴に残す最大件数。0 は履歴を記録しない（既定: 200 / Issue #89）
    pub history_max_entries: usize,
    /// 自動翻訳の結果を履歴に確定するまでの待機時間（ミリ秒、既定: 3000ms / Issue #89）
    pub history_commit_delay_ms: u64,
}

/// `history_max_entries` の上限値
pub const MAX_HISTORY_ENTRIES: usize = 1000;

/// `history_commit_delay_ms` の下限と上限（ミリ秒）
pub const MIN_HISTORY_COMMIT_DELAY_MS: u64 = 500;
pub const MAX_HISTORY_COMMIT_DELAY_MS: u64 = 30000;

/// `use_tray` の既定値。
///
/// Linux はトレイを表示しない環境（Wayland / GNOME 標準など）があり、
/// 「×」で隠すとウィンドウへ戻れなくなるため、既定では常駐しない。
/// それ以外の OS は従来どおりトレイに常駐する。
pub fn default_use_tray() -> bool {
    !cfg!(target_os = "linux")
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
            auto_translate: false,
            auto_translate_delay_ms: 800,
            use_tray: default_use_tray(),
            history_max_entries: 200,
            history_commit_delay_ms: 3000,
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
        if self.auto_translate_delay_ms < 100 || self.auto_translate_delay_ms > 10000 {
            return Err(AppError::InvalidSettings(
                "auto_translate_delay_ms must be between 100 and 10000".to_string(),
            ));
        }
        if self.history_max_entries > MAX_HISTORY_ENTRIES {
            return Err(AppError::InvalidSettings(format!(
                "history_max_entries must be between 0 and {MAX_HISTORY_ENTRIES}"
            )));
        }
        if self.history_commit_delay_ms < MIN_HISTORY_COMMIT_DELAY_MS
            || self.history_commit_delay_ms > MAX_HISTORY_COMMIT_DELAY_MS
        {
            return Err(AppError::InvalidSettings(format!(
                "history_commit_delay_ms must be between {MIN_HISTORY_COMMIT_DELAY_MS} and {MAX_HISTORY_COMMIT_DELAY_MS}"
            )));
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
            auto_translate: true,
            auto_translate_delay_ms: 500,
            use_tray: !default_use_tray(),
            history_max_entries: 50,
            history_commit_delay_ms: 1000,
        };
        settings.save(&path).unwrap();

        assert_eq!(Settings::load(&path).unwrap(), settings);
        // 一時ファイルは残らない
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn auto_translate_delay_validation() {
        let mut settings = Settings {
            auto_translate_delay_ms: 99,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());

        settings.auto_translate_delay_ms = 10001;
        assert!(settings.validate().is_err());

        settings.auto_translate_delay_ms = 100;
        assert!(settings.validate().is_ok());

        settings.auto_translate_delay_ms = 10000;
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn history_settings_defaults() {
        let settings = Settings::default();
        assert_eq!(settings.history_max_entries, 200);
        assert_eq!(settings.history_commit_delay_ms, 3000);
    }

    #[test]
    fn history_max_entries_validation() {
        let with_max = |max: usize| Settings {
            history_max_entries: max,
            ..Settings::default()
        };
        // 0 は「記録しない」として有効
        assert!(with_max(0).validate().is_ok());
        assert!(with_max(MAX_HISTORY_ENTRIES).validate().is_ok());
        assert!(with_max(MAX_HISTORY_ENTRIES + 1).validate().is_err());
    }

    #[test]
    fn history_commit_delay_validation() {
        let with_delay = |ms: u64| Settings {
            history_commit_delay_ms: ms,
            ..Settings::default()
        };
        assert!(with_delay(MIN_HISTORY_COMMIT_DELAY_MS - 1)
            .validate()
            .is_err());
        assert!(with_delay(MIN_HISTORY_COMMIT_DELAY_MS).validate().is_ok());
        assert!(with_delay(MAX_HISTORY_COMMIT_DELAY_MS).validate().is_ok());
        assert!(with_delay(MAX_HISTORY_COMMIT_DELAY_MS + 1)
            .validate()
            .is_err());
    }

    #[test]
    fn file_without_history_settings_uses_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // 履歴の項目が追加される前の設定ファイルでも、既定値で読み込める
        fs::write(&path, r#"{ "target_lang": "en" }"#).unwrap();

        let settings = Settings::load(&path).unwrap();
        assert_eq!(settings.history_max_entries, 200);
        assert_eq!(settings.history_commit_delay_ms, 3000);
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
        // use_tray が無い設定ファイルでは、OS ごとの既定値になる（ADR-007）
        assert_eq!(settings.use_tray, default_use_tray());
    }

    #[test]
    fn use_tray_default_follows_os() {
        // Linux はトレイが見えない環境があるため常駐しない。それ以外は常駐する
        if cfg!(target_os = "linux") {
            assert!(!default_use_tray());
        } else {
            assert!(default_use_tray());
        }
        assert_eq!(Settings::default().use_tray, default_use_tray());
    }

    #[test]
    fn explicit_use_tray_overrides_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        fs::write(&path, r#"{ "use_tray": false }"#).unwrap();
        assert!(!Settings::load(&path).unwrap().use_tray);

        fs::write(&path, r#"{ "use_tray": true }"#).unwrap();
        assert!(Settings::load(&path).unwrap().use_tray);
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
