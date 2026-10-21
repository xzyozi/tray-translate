use crate::error::{AppError, Result};

/// 導入済みの言語パッケージ
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModel {
    /// `en_ja` のようなペアキー
    pub pair_key: String,
    pub from: String,
    pub to: String,
}

/// `en_ja` のようなペアキーを分解する。形式が不正なら `None`。
pub fn parse_pair_key(key: &str) -> Option<InstalledModel> {
    let (from, to) = key.split_once('_')?;
    if from.is_empty() || to.is_empty() || to.contains('_') {
        return None;
    }
    Some(InstalledModel {
        pair_key: key.to_string(),
        from: from.to_string(),
        to: to.to_string(),
    })
}

/// ペアキーの一覧から導入済みモデルの一覧を作る。形式が不正なキーは除く。順序は入力のまま。
pub fn installed_models(pair_keys: &[String]) -> Vec<InstalledModel> {
    let mut models = Vec::new();
    for key in pair_keys {
        if let Some(model) = parse_pair_key(key) {
            models.push(model);
        }
    }
    models
}

/// `from` から `to` への翻訳に必要で、まだ導入されていないパッケージのペアキーを返す。
///
/// ルーターの経路選択に合わせる。
/// - 同じ言語、または直接ペアが導入済みなら、不足はない。
/// - どちらかが英語なら、直接ペアが必要。
/// - どちらも英語でなければ、英語を中継する 2 つ（`from_en` と `en_to`）のうち不足分を返す。
pub fn required_packages(installed: &[String], from: &str, to: &str) -> Vec<String> {
    let from = from.to_ascii_lowercase();
    let to = to.to_ascii_lowercase();

    if from == to {
        return Vec::new();
    }

    let direct = format!("{from}_{to}");
    if installed.contains(&direct) {
        return Vec::new();
    }
    if from == "en" || to == "en" {
        return vec![direct];
    }

    let mut missing = Vec::new();
    for key in [format!("{from}_en"), format!("en_{to}")] {
        if !installed.contains(&key) {
            missing.push(key);
        }
    }
    missing
}

/// モデルのダウンロード状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadState {
    Idle,
    Running { pair: String },
    Succeeded { pair: String },
    Failed { pair: String, message: String },
}

/// ダウンロードの状態を管理する。同時に実行できるのは 1 件だけ。
///
/// `finish_err` に渡すメッセージは、認証情報などを含まない形にしてから渡すこと。
pub struct DownloadTracker {
    state: DownloadState,
}

impl DownloadTracker {
    pub fn new() -> Self {
        Self {
            state: DownloadState::Idle,
        }
    }

    pub fn state(&self) -> &DownloadState {
        &self.state
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, DownloadState::Running { .. })
    }

    /// ダウンロードを開始する。すでに実行中ならエラー。
    pub fn start(&mut self, pair: &str) -> Result<()> {
        if let DownloadState::Running { pair: current } = &self.state {
            return Err(AppError::DownloadInProgress(current.clone()));
        }
        self.state = DownloadState::Running {
            pair: pair.to_string(),
        };
        Ok(())
    }

    /// 実行中のダウンロードを成功として終える。実行中でなければ何もしない。
    pub fn finish_ok(&mut self) {
        match std::mem::replace(&mut self.state, DownloadState::Idle) {
            DownloadState::Running { pair } => {
                self.state = DownloadState::Succeeded { pair };
            }
            other => self.state = other,
        }
    }

    /// 実行中のダウンロードを失敗として終える。実行中でなければ何もしない。
    pub fn finish_err(&mut self, message: &str) {
        match std::mem::replace(&mut self.state, DownloadState::Idle) {
            DownloadState::Running { pair } => {
                self.state = DownloadState::Failed {
                    pair,
                    message: message.to_string(),
                };
            }
            other => self.state = other,
        }
    }
}

impl Default for DownloadTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn parses_pair_keys() {
        let model = parse_pair_key("en_ja").unwrap();
        assert_eq!(model.pair_key, "en_ja");
        assert_eq!(model.from, "en");
        assert_eq!(model.to, "ja");
    }

    #[test]
    fn rejects_malformed_pair_keys() {
        assert_eq!(parse_pair_key("enja"), None);
        assert_eq!(parse_pair_key("_ja"), None);
        assert_eq!(parse_pair_key("en_"), None);
        assert_eq!(parse_pair_key("en_ja_x"), None);
        assert_eq!(parse_pair_key(""), None);
    }

    #[test]
    fn installed_models_skips_malformed_keys_and_keeps_order() {
        let models = installed_models(&keys(&["en_ja", "broken", "ja_en"]));
        let pairs: Vec<&str> = models.iter().map(|m| m.pair_key.as_str()).collect();
        assert_eq!(pairs, vec!["en_ja", "ja_en"]);
    }

    #[test]
    fn no_packages_needed_for_same_language_or_installed_direct_pair() {
        let installed = keys(&["en_ja"]);
        assert!(required_packages(&installed, "ja", "ja").is_empty());
        assert!(required_packages(&installed, "en", "ja").is_empty());
        assert!(required_packages(&installed, "EN", "JA").is_empty());
    }

    #[test]
    fn direct_package_is_required_when_one_side_is_english() {
        let installed = keys(&[]);
        assert_eq!(required_packages(&installed, "en", "ja"), keys(&["en_ja"]));
        assert_eq!(required_packages(&installed, "ja", "en"), keys(&["ja_en"]));
    }

    #[test]
    fn pivot_packages_are_required_for_non_english_pairs() {
        let installed = keys(&[]);
        assert_eq!(required_packages(&installed, "ja", "es"), keys(&["ja_en", "en_es"]));
    }

    #[test]
    fn only_missing_pivot_packages_are_reported() {
        let installed = keys(&["ja_en"]);
        assert_eq!(required_packages(&installed, "ja", "es"), keys(&["en_es"]));

        let installed = keys(&["ja_en", "en_es"]);
        assert!(required_packages(&installed, "ja", "es").is_empty());
    }

    #[test]
    fn installed_direct_pair_is_used_instead_of_pivot() {
        let installed = keys(&["ja_es"]);
        assert!(required_packages(&installed, "ja", "es").is_empty());
    }

    #[test]
    fn download_lifecycle_success() {
        let mut tracker = DownloadTracker::new();
        assert_eq!(tracker.state(), &DownloadState::Idle);
        assert!(!tracker.is_running());

        tracker.start("en_ja").unwrap();
        assert!(tracker.is_running());

        tracker.finish_ok();
        assert!(!tracker.is_running());
        assert_eq!(
            tracker.state(),
            &DownloadState::Succeeded {
                pair: "en_ja".to_string()
            }
        );
    }

    #[test]
    fn download_lifecycle_failure() {
        let mut tracker = DownloadTracker::new();
        tracker.start("en_ja").unwrap();
        tracker.finish_err("network unreachable");
        assert_eq!(
            tracker.state(),
            &DownloadState::Failed {
                pair: "en_ja".to_string(),
                message: "network unreachable".to_string()
            }
        );
    }

    #[test]
    fn second_download_is_rejected_while_running() {
        let mut tracker = DownloadTracker::new();
        tracker.start("en_ja").unwrap();

        let result = tracker.start("ja_en");
        assert!(matches!(result, Err(AppError::DownloadInProgress(ref pair)) if pair == "en_ja"));
        // 実行中の状態は変わらない
        assert_eq!(
            tracker.state(),
            &DownloadState::Running {
                pair: "en_ja".to_string()
            }
        );
    }

    #[test]
    fn can_start_again_after_finishing() {
        let mut tracker = DownloadTracker::new();
        tracker.start("en_ja").unwrap();
        tracker.finish_err("failed");
        assert!(tracker.start("en_ja").is_ok());

        tracker.finish_ok();
        assert!(tracker.start("ja_en").is_ok());
    }

    #[test]
    fn finishing_without_running_does_nothing() {
        let mut tracker = DownloadTracker::new();
        tracker.finish_ok();
        assert_eq!(tracker.state(), &DownloadState::Idle);
        tracker.finish_err("ignored");
        assert_eq!(tracker.state(), &DownloadState::Idle);
    }
}
