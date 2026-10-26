//! `hotkey` feature が無効のときの差し替え実装（ADR-007）。
//!
//! `hotkey.rs` と同じ API を持ち、`HotkeyManager::new` は常に失敗する。
//! 呼び出し側（`app.rs`）は登録失敗として扱い、ホットキーなしの通常ウィンドウで動く。
//! ホットキーはウィンドウの終了判断には関与しない。

use eframe::egui;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// ホットキーを持たない環境向けのダミー。生成できないので、値が作られることはない。
#[allow(dead_code)]
pub struct HotkeyManager;

#[allow(dead_code)]
impl HotkeyManager {
    /// 常に失敗する。理由は呼び出し側のログに出る。
    pub fn new(_ctx: egui::Context, _initial_hotkey_str: &str) -> Result<Self, BoxError> {
        Err("このビルドではグローバルホットキー機能（hotkey feature）が無効です".into())
    }

    /// 常に失敗する
    pub fn register(&mut self, _hotkey_str: &str) -> Result<(), BoxError> {
        Err("このビルドではグローバルホットキー機能（hotkey feature）が無効です".into())
    }

    /// 押下は検出されない
    pub fn poll_pressed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_always_fails() {
        assert!(HotkeyManager::new(egui::Context::default(), "Ctrl+Alt+T").is_err());
    }

    #[test]
    fn error_message_mentions_the_feature() {
        let message = HotkeyManager::new(egui::Context::default(), "Ctrl+Alt+T")
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(message.contains("hotkey"));
    }
}
