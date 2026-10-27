//! 翻訳履歴（Issue #89）。
//!
//! 画面や OS に依存しない。重複の判定、件数の上限、保存と読み込みを担う。
//! 並びは先頭が最新。保存は一時ファイルに書いてから置き換える（設定ファイルと同じ方式）。

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

/// 保存形式の版数
const FORMAT_VERSION: u32 = 1;

/// 履歴の1件。重複の判定は「翻訳元・翻訳先・原文」の一致で行う。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// 翻訳元の言語コード
    pub source_lang: String,
    /// 翻訳先の言語コード
    pub target_lang: String,
    /// 原文
    pub source: String,
    /// 訳文
    pub output: String,
    /// 最後に記録した時刻（UNIX 秒）
    pub used_at: u64,
}

impl HistoryEntry {
    /// 現在時刻を記録時刻として、エントリを作る。
    pub fn new(source_lang: String, target_lang: String, source: String, output: String) -> Self {
        Self {
            source_lang,
            target_lang,
            source,
            output,
            used_at: now_unix_secs(),
        }
    }

    /// 重複判定のキー（翻訳元・翻訳先・原文）が等しいか。言語は大文字小文字を区別せず、前後の空白は無視する。
    pub fn same_key(&self, other: &Self) -> bool {
        self.source_lang
            .trim()
            .eq_ignore_ascii_case(other.source_lang.trim())
            && self
                .target_lang
                .trim()
                .eq_ignore_ascii_case(other.target_lang.trim())
            && self.source.trim() == other.source.trim()
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// 保存ファイルの読み込み用
#[derive(Deserialize)]
struct HistoryFile {
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

/// 保存ファイルの書き出し用
#[derive(Serialize)]
struct HistoryFileRef<'a> {
    version: u32,
    entries: &'a [HistoryEntry],
}

/// 翻訳履歴。先頭が最新。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    entries: Vec<HistoryEntry>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新しい順のエントリ一覧
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 1件を記録する。変更があれば true を返す。
    ///
    /// - `max_entries` が 0 のときは記録しない。
    /// - 原文が空（空白のみ）のときは記録しない。
    /// - キーが同じエントリが既にあれば、追加せずに先頭へ移し、訳文と時刻を更新する。
    ///   既に先頭にあり、訳文も同じなら何も変えない（無駄な書き込みを避ける）。
    /// - 上限を超えた分は、古いものから削除する。
    pub fn record(&mut self, entry: HistoryEntry, max_entries: usize) -> bool {
        if max_entries == 0 || entry.source.trim().is_empty() {
            return false;
        }

        let entry = HistoryEntry {
            source_lang: entry.source_lang.trim().to_ascii_lowercase(),
            target_lang: entry.target_lang.trim().to_ascii_lowercase(),
            source: entry.source.trim().to_string(),
            output: entry.output,
            used_at: entry.used_at,
        };

        let changed = match self.entries.iter().position(|e| e.same_key(&entry)) {
            Some(0) if self.entries[0].output == entry.output => false,
            Some(index) => {
                self.entries.remove(index);
                self.entries.insert(0, entry);
                true
            }
            None => {
                self.entries.insert(0, entry);
                true
            }
        };

        let trimmed = self.entries.len() > max_entries;
        self.entries.truncate(max_entries);
        changed || trimmed
    }

    /// キーが同じエントリを削除する。削除したら true を返す。
    pub fn remove(&mut self, target: &HistoryEntry) -> bool {
        match self.entries.iter().position(|e| e.same_key(target)) {
            Some(index) => {
                self.entries.remove(index);
                true
            }
            None => false,
        }
    }

    /// すべて削除する。削除するものがあれば true を返す。
    pub fn clear(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        self.entries.clear();
        true
    }

    /// 履歴ファイルを読み込む。ファイルが無ければ空の履歴を返す。
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let text = fs::read_to_string(path)?;
        let file: HistoryFile = serde_json::from_str(&text)?;
        Ok(Self {
            entries: file.entries,
        })
    }

    /// 履歴ファイルを読み込む。読み込めない場合は、そのファイルを `.corrupt` に退避して空で始める。
    ///
    /// 履歴が壊れていても、アプリの起動は妨げない。
    pub fn load_or_recover(path: &Path) -> Self {
        match Self::load(path) {
            Ok(history) => history,
            Err(error) => {
                warn!(
                    path = %path.display(),
                    error = %error,
                    "failed to load history; starting with an empty history"
                );
                let backup = path.with_extension("json.corrupt");
                if let Err(rename_error) = fs::rename(path, &backup) {
                    warn!(
                        path = %path.display(),
                        error = %rename_error,
                        "failed to move the unreadable history file aside"
                    );
                }
                Self::new()
            }
        }
    }

    /// 履歴ファイルを保存する。一時ファイルに書いてから置き換える。
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let file = HistoryFileRef {
            version: FORMAT_VERSION,
            entries: &self.entries,
        };
        let json = serde_json::to_string_pretty(&file)?;
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

    fn entry(source: &str, output: &str) -> HistoryEntry {
        HistoryEntry {
            source_lang: "en".to_string(),
            target_lang: "ja".to_string(),
            source: source.to_string(),
            output: output.to_string(),
            used_at: 100,
        }
    }

    fn sources(history: &History) -> Vec<&str> {
        history
            .entries()
            .iter()
            .map(|e| e.source.as_str())
            .collect()
    }

    #[test]
    fn new_entries_are_added_to_the_front() {
        let mut history = History::new();
        assert!(history.record(entry("one", "1"), 10));
        assert!(history.record(entry("two", "2"), 10));
        assert_eq!(sources(&history), vec!["two", "one"]);
    }

    #[test]
    fn zero_limit_records_nothing() {
        let mut history = History::new();
        assert!(!history.record(entry("one", "1"), 0));
        assert!(history.is_empty());
    }

    #[test]
    fn blank_source_is_not_recorded() {
        let mut history = History::new();
        assert!(!history.record(entry("  \n ", "x"), 10));
        assert!(history.is_empty());
    }

    #[test]
    fn duplicate_moves_to_front_instead_of_adding() {
        let mut history = History::new();
        history.record(entry("one", "1"), 10);
        history.record(entry("two", "2"), 10);

        // 古い方と同じキーを記録すると、追加せずに先頭へ移る
        let mut again = entry("one", "1-new");
        again.used_at = 200;
        assert!(history.record(again, 10));

        assert_eq!(history.len(), 2);
        assert_eq!(sources(&history), vec!["one", "two"]);
        // 訳文と時刻は新しい内容に更新される
        assert_eq!(history.entries()[0].output, "1-new");
        assert_eq!(history.entries()[0].used_at, 200);
    }

    #[test]
    fn identical_front_entry_is_not_a_change() {
        let mut history = History::new();
        history.record(entry("one", "1"), 10);

        // 先頭と同じキーで訳文も同じなら、変更なし（書き込みを起こさない）
        let mut again = entry("one", "1");
        again.used_at = 999;
        assert!(!history.record(again, 10));
        assert_eq!(history.entries()[0].used_at, 100);
    }

    #[test]
    fn duplicate_key_ignores_case_of_language_and_surrounding_spaces() {
        let mut history = History::new();
        history.record(entry("hello", "x"), 10);

        let mut same = entry("  hello ", "y");
        same.source_lang = " EN ".to_string();
        history.record(same, 10);

        assert_eq!(history.len(), 1);
        assert_eq!(history.entries()[0].output, "y");
        // 保存時に正規化される
        assert_eq!(history.entries()[0].source_lang, "en");
        assert_eq!(history.entries()[0].source, "hello");
    }

    #[test]
    fn language_direction_distinguishes_entries() {
        let mut history = History::new();
        history.record(entry("hello", "x"), 10);

        let mut reverse = entry("hello", "y");
        reverse.source_lang = "ja".to_string();
        reverse.target_lang = "en".to_string();
        history.record(reverse, 10);

        assert_eq!(history.len(), 2);
    }

    #[test]
    fn oldest_entries_are_dropped_beyond_the_limit() {
        let mut history = History::new();
        for i in 0..5 {
            history.record(entry(&format!("s{i}"), "o"), 3);
        }
        assert_eq!(sources(&history), vec!["s4", "s3", "s2"]);
    }

    #[test]
    fn lowering_the_limit_trims_on_the_next_record() {
        let mut history = History::new();
        for i in 0..5 {
            history.record(entry(&format!("s{i}"), "o"), 10);
        }
        // 上限を下げた後の記録で、超過分がまとめて削除される
        assert!(history.record(entry("s5", "o"), 2));
        assert_eq!(sources(&history), vec!["s5", "s4"]);
    }

    #[test]
    fn remove_deletes_only_the_matching_entry() {
        let mut history = History::new();
        history.record(entry("one", "1"), 10);
        history.record(entry("two", "2"), 10);

        assert!(history.remove(&entry("one", "ignored")));
        assert_eq!(sources(&history), vec!["two"]);
        assert!(!history.remove(&entry("one", "1")));
    }

    #[test]
    fn clear_reports_whether_anything_was_removed() {
        let mut history = History::new();
        assert!(!history.clear());
        history.record(entry("one", "1"), 10);
        assert!(history.clear());
        assert!(history.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("history.json");

        let mut history = History::new();
        history.record(entry("one", "1"), 10);
        history.record(entry("日本語", "Japanese"), 10);
        history.save(&path).unwrap();

        assert_eq!(History::load(&path).unwrap(), history);
        // 一時ファイルは残らない
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn missing_file_loads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::load(&dir.path().join("history.json")).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn broken_file_is_an_error_for_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(&path, "not json").unwrap();
        assert!(History::load(&path).is_err());
    }

    #[test]
    fn broken_file_is_moved_aside_and_history_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(&path, "not json").unwrap();

        let history = History::load_or_recover(&path);

        assert!(history.is_empty());
        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(path.with_extension("json.corrupt")).unwrap(),
            "not json"
        );
    }

    #[test]
    fn file_without_entries_loads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(&path, r#"{ "version": 1 }"#).unwrap();
        assert!(History::load(&path).unwrap().is_empty());
    }

    #[test]
    fn save_failure_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        // 保存先をディレクトリにして rename を失敗させる
        let path = dir.path().join("history.json");
        fs::create_dir(&path).unwrap();

        assert!(History::new().save(&path).is_err());
        assert!(!path.with_extension("json.tmp").exists());
    }
}
