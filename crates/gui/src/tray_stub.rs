//! `tray` feature が無効のときの差し替え実装（ADR-007）。
//!
//! `tray.rs` と同じ API を持ち、`TrayManager::new` は常に失敗する。
//! 呼び出し側（`app.rs`）は初期化失敗として扱い、トレイなしの通常ウィンドウで動く。
//! このとき「×」ボタンはプロセスの終了になる（`lifecycle` の判断）。

use eframe::egui;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// トレイからの要求操作（`tray.rs` と同じ定義）
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// 翻訳ウィンドウを表示・最前面化
    Show,
    /// アプリケーション終了
    Quit,
}

/// トレイを持たない環境向けのダミー。生成できないので、値が作られることはない。
#[allow(dead_code)]
pub struct TrayManager;

impl TrayManager {
    /// 常に失敗する。理由は呼び出し側のログに出る。
    pub fn new(_ctx: egui::Context) -> Result<Self, BoxError> {
        Err("このビルドではタスクトレイ機能（tray feature）が無効です".into())
    }

    /// 常に操作なし
    pub fn poll_events(&self) -> Option<TrayAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_always_fails() {
        assert!(TrayManager::new(egui::Context::default()).is_err());
    }

    #[test]
    fn error_message_mentions_the_feature() {
        let message = TrayManager::new(egui::Context::default())
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(message.contains("tray"));
    }
}
