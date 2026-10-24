//! 図形描画（Painter）によるベクターアイコンとインラインボタン（ADR-006 / Issue #59）。
//!
//! フォント文字に依存せず、Painter の直線・矩形で描画することで
//! 環境依存の文字化け（豆腐）を完全に防止する。

use crate::theme;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2, Widget};

/// アイコン種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// クリア（×印）
    Clear,
    /// コピー（重なる2枚のシート）
    Copy,
    /// チェック（✓印、コピー完了通知用）
    Check,
}

/// インラインアイコンボタン
pub struct IconButton<'a> {
    icon: Icon,
    tooltip: &'a str,
    size: Vec2,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: Icon, tooltip: &'a str) -> Self {
        Self {
            icon,
            tooltip,
            size: Vec2::splat(22.0),
        }
    }

    #[allow(dead_code)]
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }
}

impl<'a> Widget for IconButton<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(self.size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);

            // 背景と枠線を描画（テキストの上に重なっても文字が透けないよう不透明背景を敷く）
            let bg_color = if response.hovered() || response.has_focus() {
                visuals.bg_fill
            } else {
                theme::BG_PANEL
            };
            let stroke = if response.has_focus() {
                ui.visuals().selection.stroke
            } else if response.hovered() {
                visuals.bg_stroke
            } else {
                Stroke::new(1.0, Color32::from_rgb(0x3A, 0x3A, 0x3D))
            };

            let painter = ui.painter();
            painter.rect(rect, 4.0, bg_color, stroke, StrokeKind::Inside);

            // アイコンの線の色
            let icon_stroke = Stroke::new(
                1.5,
                if response.hovered() || response.has_focus() {
                    visuals.fg_stroke.color
                } else {
                    theme::TEXT_PRIMARY
                },
            );

            let center = rect.center();

            match self.icon {
                Icon::Clear => {
                    // ×印を描画
                    let s = 4.5;
                    painter.line_segment(
                        [
                            Pos2::new(center.x - s, center.y - s),
                            Pos2::new(center.x + s, center.y + s),
                        ],
                        icon_stroke,
                    );
                    painter.line_segment(
                        [
                            Pos2::new(center.x - s, center.y + s),
                            Pos2::new(center.x + s, center.y - s),
                        ],
                        icon_stroke,
                    );
                }
                Icon::Copy => {
                    // 重なる2枚のシートを描画
                    // 奥のシート（右上）
                    let back_min = Pos2::new(center.x - 2.0, center.y - 5.5);
                    let sheet_size = Vec2::new(7.0, 8.5);
                    let back_rect = Rect::from_min_size(back_min, sheet_size);
                    painter.rect_stroke(back_rect, 1.0, icon_stroke, StrokeKind::Inside);

                    // 手前のシート（左下）
                    let front_min = Pos2::new(center.x - 5.5, center.y - 2.0);
                    let front_rect = Rect::from_min_size(front_min, sheet_size);
                    // 奥のシートを隠すために手前シートの背景を塗りつぶす
                    painter.rect_filled(front_rect, 1.0, bg_color);
                    painter.rect_stroke(front_rect, 1.0, icon_stroke, StrokeKind::Inside);
                }
                Icon::Check => {
                    // ✓印を描画
                    let check_stroke = Stroke::new(1.8, theme::SUCCESS);
                    let p1 = Pos2::new(center.x - 5.0, center.y);
                    let p2 = Pos2::new(center.x - 1.5, center.y + 3.5);
                    let p3 = Pos2::new(center.x + 5.0, center.y - 3.5);
                    painter.line_segment([p1, p2], check_stroke);
                    painter.line_segment([p2, p3], check_stroke);
                }
            }
        }

        if !self.tooltip.is_empty() {
            response.on_hover_text(self.tooltip)
        } else {
            response
        }
    }
}

/// 簡潔にアイコンボタンを追加するヘルパー関数
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tooltip: &str) -> egui::Response {
    ui.add(IconButton::new(icon, tooltip))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_types_are_distinct() {
        assert_ne!(Icon::Clear, Icon::Copy);
        assert_ne!(Icon::Copy, Icon::Check);
        assert_ne!(Icon::Clear, Icon::Check);
    }

    #[test]
    fn icon_button_creation() {
        let btn = IconButton::new(Icon::Clear, "クリア");
        assert_eq!(btn.icon, Icon::Clear);
        assert_eq!(btn.tooltip, "クリア");
        assert_eq!(btn.size, Vec2::splat(22.0));
    }
}
