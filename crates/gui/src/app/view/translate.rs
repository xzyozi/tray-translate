//! 翻訳タブの描画（言語バー、原文・訳文エリア、レイアウト切替）。

use crate::app::languages::{collect_available_languages, language_display_name};
use crate::app::state::{ActiveTab, AppStatus};
use crate::app::GuiApp;
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui;
use std::time::{Duration, Instant};
use tray_translate_app::{models::required_packages, Command};

/// 左右分割と上下配置の境界幅（ADR-006: 640px 以上で左右分割、未満で上下配置）
pub const SIDE_BY_SIDE_BREAKPOINT: f32 = 640.0;

/// 画面幅に基づいて左右分割レイアウトを採用するかどうかを判定する。
pub fn is_side_by_side(window_width: f32) -> bool {
    window_width >= SIDE_BY_SIDE_BREAKPOINT
}

/// 言語ドロップダウンを表示するヘルパー。値が変更された場合は true を返す。
fn render_language_combo(
    ui: &mut egui::Ui,
    id_salt: &str,
    current_lang: &mut String,
    candidates: &[String],
    width: Option<f32>,
) -> bool {
    let mut changed = false;
    let selected_text = language_display_name(current_lang);
    let mut combo = egui::ComboBox::from_id_salt(id_salt).selected_text(selected_text);
    if let Some(w) = width {
        combo = combo.width(w);
    }
    combo.show_ui(ui, |ui| {
        for lang in candidates {
            let display = language_display_name(lang);
            let is_selected = current_lang.trim().eq_ignore_ascii_case(lang);
            if ui.selectable_label(is_selected, display).clicked() && !is_selected {
                *current_lang = lang.clone();
                changed = true;
            }
        }
    });
    changed
}

impl GuiApp {
    /// 左右分割時の言語選択バー（カラム幅と中央セパレータ位置に完全追従する自動幅調整）
    #[allow(clippy::too_many_arguments)]
    fn render_language_bar_wide(
        &mut self,
        ui: &mut egui::Ui,
        col_width: f32,
        can_swap: bool,
        reverse_missing: &[String],
        reverse_from: &str,
        reverse_to: &str,
        missing: &[String],
        src_candidates: &[String],
        dst_candidates: &[String],
    ) {
        ui.horizontal(|ui| {
            // 左側（原文エリアの上部）: 翻訳元言語 ComboBox（カラム幅いっぱいに自動拡大）
            ui.allocate_ui(egui::vec2(col_width, 24.0), |ui| {
                ui.horizontal(|ui| {
                    ui.label("翻訳元:");
                    let combo_width = (ui.available_width() - 8.0).max(100.0);
                    if render_language_combo(
                        ui,
                        "src_lang_combo",
                        &mut self.source_lang,
                        src_candidates,
                        Some(combo_width),
                    ) {
                        self.update_settings();
                        self.request_tracker.invalidate();
                        if self.auto_translate && !self.input.trim().is_empty() {
                            self.pending_auto_translate = Some(Instant::now());
                        }
                    }
                });
            });

            // 中央（セパレータの上部）: 言語入れ替えボタン
            let swap_btn = egui::Button::new("⇆");
            let swap_res = if can_swap {
                ui.add_enabled(can_swap, swap_btn)
                    .on_hover_text("言語を入れ替える（テキストは保持）")
            } else if reverse_from == reverse_to && !reverse_from.is_empty() {
                ui.add_enabled(can_swap, swap_btn)
                    .on_disabled_hover_text("翻訳元と翻訳先が同じ言語です")
            } else {
                let missing_text = if reverse_missing.is_empty() {
                    "モデルが未設定です".to_string()
                } else {
                    format!(
                        "逆方向（{} → {}）のモデルが未導入です: {}",
                        reverse_from,
                        reverse_to,
                        reverse_missing.join(", ")
                    )
                };
                ui.add_enabled(can_swap, swap_btn)
                    .on_disabled_hover_text(missing_text)
            };

            if swap_res.clicked() {
                std::mem::swap(&mut self.source_lang, &mut self.target_lang);
                self.update_settings();
                self.request_tracker.invalidate();
                if self.auto_translate && !self.input.trim().is_empty() {
                    self.pending_auto_translate = Some(Instant::now());
                }
            }

            // 右側（訳文エリアの上部）: 翻訳先言語 ComboBox ＆ 未導入警告（右カラム幅に追従）
            let right_width = ui.available_width();
            ui.allocate_ui(egui::vec2(right_width, 24.0), |ui| {
                ui.horizontal(|ui| {
                    ui.label("翻訳先:");
                    let combo_width = if missing.is_empty() {
                        (ui.available_width() - 8.0).max(100.0)
                    } else {
                        110.0
                    };
                    if render_language_combo(
                        ui,
                        "dst_lang_combo",
                        &mut self.target_lang,
                        dst_candidates,
                        Some(combo_width),
                    ) {
                        self.update_settings();
                        self.request_tracker.invalidate();
                        if self.auto_translate && !self.input.trim().is_empty() {
                            self.pending_auto_translate = Some(Instant::now());
                        }
                    }

                    if !missing.is_empty() {
                        ui.separator();
                        ui.colored_label(theme::WARNING, format!("未導入: {}", missing.join(", ")));
                        if ui.button("モデル管理で取得").clicked() {
                            self.active_tab = ActiveTab::Models;
                        }
                    }
                });
            });
        });
    }

    /// 上下配置時の言語選択バー（Narrow）
    #[allow(clippy::too_many_arguments)]
    fn render_language_bar_narrow(
        &mut self,
        ui: &mut egui::Ui,
        can_swap: bool,
        reverse_missing: &[String],
        reverse_from: &str,
        reverse_to: &str,
        missing: &[String],
        src_candidates: &[String],
        dst_candidates: &[String],
    ) {
        ui.horizontal(|ui| {
            ui.label("翻訳元:");
            if render_language_combo(
                ui,
                "src_lang_combo",
                &mut self.source_lang,
                src_candidates,
                Some(110.0),
            ) {
                self.update_settings();
                self.request_tracker.invalidate();
                if self.auto_translate && !self.input.trim().is_empty() {
                    self.pending_auto_translate = Some(Instant::now());
                }
            }

            let swap_btn = egui::Button::new("⇆");
            let swap_res = if can_swap {
                ui.add_enabled(can_swap, swap_btn)
                    .on_hover_text("言語を入れ替える（テキストは保持）")
            } else if reverse_from == reverse_to && !reverse_from.is_empty() {
                ui.add_enabled(can_swap, swap_btn)
                    .on_disabled_hover_text("翻訳元と翻訳先が同じ言語です")
            } else {
                let missing_text = if reverse_missing.is_empty() {
                    "モデルが未設定です".to_string()
                } else {
                    format!(
                        "逆方向（{} → {}）のモデルが未導入です: {}",
                        reverse_from,
                        reverse_to,
                        reverse_missing.join(", ")
                    )
                };
                ui.add_enabled(can_swap, swap_btn)
                    .on_disabled_hover_text(missing_text)
            };

            if swap_res.clicked() {
                std::mem::swap(&mut self.source_lang, &mut self.target_lang);
                self.update_settings();
                self.request_tracker.invalidate();
                if self.auto_translate && !self.input.trim().is_empty() {
                    self.pending_auto_translate = Some(Instant::now());
                }
            }

            ui.label("翻訳先:");
            if render_language_combo(
                ui,
                "dst_lang_combo",
                &mut self.target_lang,
                dst_candidates,
                Some(110.0),
            ) {
                self.update_settings();
                self.request_tracker.invalidate();
                if self.auto_translate && !self.input.trim().is_empty() {
                    self.pending_auto_translate = Some(Instant::now());
                }
            }
        });

        if !missing.is_empty() {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.colored_label(theme::WARNING, format!("未導入: {}", missing.join(", ")));
                if ui.button("モデル管理で取得").clicked() {
                    self.active_tab = ActiveTab::Models;
                }
            });
        }
    }

    /// 原文入力エリアと操作ボタン（クリアボタンはテキストエリア内右下にインライン配置）
    fn render_source_section(&mut self, ui: &mut egui::Ui, rows: usize) {
        ui.label("翻訳するテキスト");
        let edit_res = ui.add(
            egui::TextEdit::multiline(&mut self.input)
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        );

        if edit_res.changed() {
            // 入力が編集されたので、保留中の自動翻訳の結果は途中経過として履歴に残さない（Issue #89）
            self.history_hold.discard();
            self.request_tracker.invalidate();
            if self.auto_translate && !self.input.trim().is_empty() {
                self.pending_auto_translate = Some(Instant::now());
            } else {
                self.pending_auto_translate = None;
            }
        }

        // テキストエリア右下にクリア（×）ボタンをインライン配置（テキストがあるときのみ表示）
        if !self.input.is_empty() {
            let btn_size = egui::vec2(22.0, 22.0);
            let btn_rect = egui::Rect::from_min_size(
                edit_res.rect.max - btn_size - egui::vec2(6.0, 6.0),
                btn_size,
            );
            let clear_clicked = ui
                .put(btn_rect, |ui: &mut egui::Ui| {
                    icons::icon_button(ui, Icon::Clear, "入力をクリア")
                })
                .clicked();
            if clear_clicked {
                // クリアは翻訳結果を読み終えた操作とみなし、保留中の結果は確定する
                self.commit_pending_history();
                self.input.clear();
                self.request_tracker.invalidate();
                self.pending_auto_translate = None;
            }
        }

        ui.add_space(4.0);
        let btn_text = if self.is_translating {
            "翻訳中…"
        } else {
            "翻訳"
        };
        let btn = ui.add_enabled(!self.is_translating, egui::Button::new(btn_text));
        if btn.clicked() {
            self.trigger_translation(false);
        }
    }

    /// 訳文表示エリアと操作ボタン（コピーボタンはテキストエリア内右下にインライン配置）
    fn render_target_section(&mut self, ui: &mut egui::Ui, rows: usize) {
        ui.label("翻訳結果");
        let edit_res = ui.add(
            egui::TextEdit::multiline(&mut self.output.as_str())
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        );

        // テキストエリア右下にコピーボタンをインライン配置
        let is_recent_copied = self
            .last_copied_at
            .map(|t| t.elapsed() < Duration::from_secs(2))
            .unwrap_or(false);

        let (icon, tooltip) = if is_recent_copied {
            (Icon::Check, "クリップボードにコピーしました！")
        } else {
            (Icon::Copy, "結果をクリップボードにコピー")
        };

        let can_copy = !self.output.is_empty();
        let btn_size = egui::vec2(22.0, 22.0);
        let btn_rect = egui::Rect::from_min_size(
            edit_res.rect.max - btn_size - egui::vec2(6.0, 6.0),
            btn_size,
        );

        let copy_res = ui.put(btn_rect, |ui: &mut egui::Ui| {
            ui.add_enabled_ui(can_copy, |ui| icons::icon_button(ui, icon, tooltip))
                .inner
        });

        if copy_res.clicked() && can_copy {
            let _ = self
                .cmd_tx
                .send(Command::NoteClipboardWritten(self.output.clone()));
            ui.ctx().copy_text(self.output.clone());
            self.status = AppStatus::Info("結果をクリップボードにコピーしました".to_string());
            self.last_copied_at = Some(Instant::now());
        }

        // コピー表示中（2秒以内）ならチェック表示解除のために再描画をスケジュール
        if is_recent_copied {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
    }

    /// 翻訳メイン画面（ADR-006: 640px 以上で左右分割、未満で上下配置、自動幅調整対応）
    pub(in crate::app) fn render_translate_tab(&mut self, ui: &mut egui::Ui) {
        let window_width = ui.ctx().input(|i| {
            i.viewport()
                .inner_rect
                .map(|r| r.width())
                .unwrap_or(ui.available_width())
        });
        let is_wide = is_side_by_side(window_width);

        let src_candidates = collect_available_languages(&self.installed, &self.source_lang);
        let dst_candidates = collect_available_languages(&self.installed, &self.target_lang);

        let reverse_from = self.target_lang.trim().to_ascii_lowercase();
        let reverse_to = self.source_lang.trim().to_ascii_lowercase();
        let reverse_missing = required_packages(&self.installed, &reverse_from, &reverse_to);
        let can_swap = reverse_missing.is_empty()
            && !reverse_from.is_empty()
            && !reverse_to.is_empty()
            && reverse_from != reverse_to;

        let missing = self.missing_packages();

        if is_wide {
            // 左右分割レイアウト (Side-by-Side: カラム幅・セパレータ・ドロップダウン連動自動幅調整)
            let item_spacing = ui.spacing().item_spacing.x;
            let available_width = ui.available_width();
            let sep_width = 1.0;
            let col_width = ((available_width - item_spacing * 2.0 - sep_width) * 0.5)
                .max(180.0)
                .floor();

            self.render_language_bar_wide(
                ui,
                col_width,
                can_swap,
                &reverse_missing,
                &reverse_from,
                &reverse_to,
                &missing,
                &src_candidates,
                &dst_candidates,
            );
            ui.add_space(6.0);

            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(col_width, ui.available_height()),
                    egui::Layout::top_down_justified(egui::Align::Min),
                    |ui| {
                        self.render_source_section(ui, 8);
                    },
                );

                ui.separator();

                let right_width = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(right_width, ui.available_height()),
                    egui::Layout::top_down_justified(egui::Align::Min),
                    |ui| {
                        self.render_target_section(ui, 8);
                    },
                );
            });
        } else {
            // 上下配置レイアウト (Narrow)
            self.render_language_bar_narrow(
                ui,
                can_swap,
                &reverse_missing,
                &reverse_from,
                &reverse_to,
                &missing,
                &src_candidates,
                &dst_candidates,
            );
            ui.add_space(8.0);
            self.render_source_section(ui, 4);
            ui.add_space(8.0);
            self.render_target_section(ui, 4);
        }

        // 翻訳エリアの下に履歴を置く（Issue #89）
        ui.add_space(8.0);
        self.render_history_section(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_by_side_breakpoint_check() {
        assert!(is_side_by_side(640.0));
        assert!(is_side_by_side(800.0));
        assert!(!is_side_by_side(639.9));
        assert!(!is_side_by_side(420.0));
    }
}
