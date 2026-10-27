//! 履歴の描画（翻訳タブの下に置く折りたたみ領域、Issue #89）。

use crate::app::GuiApp;
use crate::theme;
use eframe::egui;
use tray_translate_app::{Command, HistoryEntry};

/// 一覧に表示する原文の最大文字数
const PREVIEW_CHARS: usize = 30;

/// 一覧に出す原文の要約。最初の空でない行を、一定の文字数までに切り詰める。
fn preview(text: &str) -> String {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let first = lines.next().unwrap_or("");
    let has_more_lines = lines.next().is_some();

    let mut chars = first.chars();
    let head: String = chars.by_ref().take(PREVIEW_CHARS).collect();
    if chars.next().is_some() || has_more_lines {
        format!("{head}…")
    } else {
        head
    }
}

impl GuiApp {
    /// 履歴の一覧。各行に「復元」と「削除」、上部に「全消去」を置く。
    pub(in crate::app) fn render_history_section(&mut self, ui: &mut egui::Ui) {
        let mut restore: Option<HistoryEntry> = None;
        let mut delete: Option<HistoryEntry> = None;
        let title = format!("履歴 ({})", self.history.len());

        // 件数でタイトルが変わっても開閉の状態を保つため、ID は固定する
        egui::CollapsingHeader::new(title)
            .id_salt("history_section")
            .default_open(false)
            .show(ui, |ui| {
                ui.small("クリップボード監視を有効にしていると、コピーした内容も履歴に残ります。");
                if self.history_max_entries == 0 {
                    ui.small("履歴の記録は無効です（設定で上限件数を 0 にしています）。");
                }

                if self.history.is_empty() {
                    self.confirm_clear_history = false;
                    ui.small("履歴はまだありません。");
                    return;
                }

                ui.horizontal(|ui| {
                    if self.confirm_clear_history {
                        ui.colored_label(theme::WARNING, "履歴をすべて消去しますか？");
                        if ui.button("消去する").clicked() {
                            let _ = self.cmd_tx.send(Command::ClearHistory);
                            self.confirm_clear_history = false;
                        }
                        if ui.button("キャンセル").clicked() {
                            self.confirm_clear_history = false;
                        }
                    } else if ui.button("全消去").clicked() {
                        self.confirm_clear_history = true;
                    }
                });

                egui::ScrollArea::vertical()
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for entry in &self.history {
                            ui.horizontal(|ui| {
                                ui.label(format!("{} → {}", entry.source_lang, entry.target_lang));
                                ui.label(preview(&entry.source))
                                    .on_hover_text(format!("{}\n→ {}", entry.source, entry.output));
                                if ui.button("復元").clicked() {
                                    restore = Some(entry.clone());
                                }
                                if ui.button("削除").clicked() {
                                    delete = Some(entry.clone());
                                }
                            });
                        }
                    });
            });

        if let Some(entry) = delete {
            let _ = self.cmd_tx.send(Command::DeleteHistory(entry));
        }
        if let Some(entry) = restore {
            self.restore_history(&entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_keeps_short_text_as_is() {
        assert_eq!(preview("hello"), "hello");
    }

    #[test]
    fn preview_truncates_long_text() {
        let text = "a".repeat(PREVIEW_CHARS + 5);
        let expected = format!("{}…", "a".repeat(PREVIEW_CHARS));
        assert_eq!(preview(&text), expected);
    }

    #[test]
    fn preview_does_not_truncate_text_of_exact_limit() {
        let text = "あ".repeat(PREVIEW_CHARS);
        assert_eq!(preview(&text), text);
    }

    #[test]
    fn preview_uses_the_first_non_empty_line_and_marks_the_rest() {
        assert_eq!(preview("\n  first line \nsecond"), "first line…");
        assert_eq!(preview("single\n\n"), "single");
    }

    #[test]
    fn preview_of_blank_text_is_empty() {
        assert_eq!(preview("  \n "), "");
    }
}
