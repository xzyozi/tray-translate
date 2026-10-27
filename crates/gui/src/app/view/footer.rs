//! 画面最下部のステータスバーの描画。

use crate::app::state::AppStatus;
use crate::app::GuiApp;
use crate::theme;
use eframe::egui;

impl GuiApp {
    /// 画面最下部のステータスバー（ADR-006 / Issue #61）
    pub(in crate::app) fn render_footer(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // 左側: 状態表示（スピナー/アイコン + メッセージ）
            match &self.status {
                AppStatus::Busy(msg) => {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.colored_label(theme::ACCENT, msg);
                }
                AppStatus::Error(msg) => {
                    ui.colored_label(theme::ERROR, "⚠");
                    ui.colored_label(theme::ERROR, msg);
                }
                AppStatus::Info(msg) => {
                    ui.colored_label(theme::TEXT_WEAK, "●");
                    ui.colored_label(theme::TEXT_WEAK, msg);
                }
            }

            // 右側: 導入済みモデル（長い場合も折り返し/省略でレイアウト破綻を防止）
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let models_text = if self.installed.is_empty() {
                    "導入済み: なし".to_string()
                } else {
                    format!("導入済み: {}", self.installed.join(", "))
                };
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(models_text)
                            .size(11.0)
                            .color(theme::TEXT_WEAK),
                    )
                    .truncate(),
                );
            });
        });
    }
}
