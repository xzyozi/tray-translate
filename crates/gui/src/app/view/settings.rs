//! 設定タブの描画。

use crate::app::GuiApp;
use eframe::egui;

impl GuiApp {
    /// 設定画面
    pub(in crate::app) fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("環境設定");
        ui.separator();

        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([12.0, 10.0])
            .show(ui, |ui| {
                ui.label("翻訳元言語コード:");
                ui.add(egui::TextEdit::singleline(&mut self.source_lang).desired_width(60.0));
                ui.end_row();

                ui.label("翻訳先言語コード:");
                ui.add(egui::TextEdit::singleline(&mut self.target_lang).desired_width(60.0));
                ui.end_row();

                ui.label("グローバルホットキー:");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.hotkey_str).desired_width(120.0));
                    ui.small("（例: Ctrl+Shift+T, Ctrl+Alt+T）");
                });
                ui.end_row();

                ui.label("アイドル時モデル解放:");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.idle_unload_secs, 0..=1800)
                            .text("秒 (0で常時保持)"),
                    );
                });
                ui.end_row();

                ui.label("クリップボード監視:");
                ui.checkbox(
                    &mut self.watch_clipboard,
                    "コピーされたテキストを自動翻訳する",
                );
                ui.end_row();

                ui.label("翻訳結果の自動コピー:");
                ui.checkbox(
                    &mut self.copy_result,
                    "翻訳完了時に結果をクリップボードに書き戻す",
                );
                ui.end_row();

                ui.label("入力後の自動翻訳:");
                ui.checkbox(&mut self.auto_translate, "タイピング停止後に自動で翻訳する");
                ui.end_row();

                ui.label("自動翻訳の待機時間:");
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        self.auto_translate,
                        egui::Slider::new(&mut self.auto_translate_delay_ms, 100..=3000)
                            .text("ミリ秒 (既定: 800ms)"),
                    );
                });
                ui.end_row();

                ui.label("トレイ常駐:");
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.use_tray, "「×」でタスクトレイに常駐する");
                    ui.small(
                        "オフにすると「×」で終了します。トレイアイコンの表示切替は再起動後に反映されます",
                    );
                });
                ui.end_row();

                ui.label("文字数制限:");
                ui.horizontal(|ui| {
                    ui.label("最小:");
                    ui.add(egui::DragValue::new(&mut self.min_chars).range(1..=100));
                    ui.label("最大:");
                    ui.add(egui::DragValue::new(&mut self.max_chars).range(100..=50000));
                });
                ui.end_row();
            });

        ui.add_space(12.0);
        if ui.button("設定を保存").clicked() {
            self.update_settings();
        }
    }
}
