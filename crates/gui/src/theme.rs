//! アプリ全体のテーマ（色・余白・角丸）。
//!
//! 色と寸法はこのファイルに定数でまとめ、`apply` で egui に適用する。
//! 画面側の個別の色指定（警告色など）も、ここの定数を使う。

use eframe::egui::{
    self, style::WidgetVisuals, Color32, CornerRadius, Margin, Stroke, Style, Theme,
    ThemePreference, Vec2,
};

/// 背景（パネル）。純黒を避けた、わずかに明るいダークグレー
pub const BG_PANEL: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
/// 入力欄などの背景。パネルより少し暗くして、入力できる領域を示す
pub const BG_INPUT: Color32 = Color32::from_rgb(0x17, 0x17, 0x17);
/// 縞模様などの、背景に近い面
pub const BG_FAINT: Color32 = Color32::from_rgb(0x25, 0x25, 0x26);
/// 本文の文字色
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xE6, 0xE6, 0xE6);
/// 補助的な文字色（ステータスなど）
pub const TEXT_WEAK: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x9A);
/// 強調色（選択、押下状態）。トレイアイコンの青に合わせる
pub const ACCENT: Color32 = Color32::from_rgb(0x2B, 0x6C, 0xB0);
/// 警告（未導入のモデルなど）
pub const WARNING: Color32 = Color32::from_rgb(0xE0, 0x9A, 0x2B);
/// エラー
pub const ERROR: Color32 = Color32::from_rgb(0xF0, 0x6A, 0x6A);
/// 成功（導入済みなど）
pub const SUCCESS: Color32 = Color32::from_rgb(0x4C, 0xC2, 0x7A);

/// ボタン・入力欄などの角丸
const RADIUS_WIDGET: u8 = 6;
/// ポップアップ・メニューの角丸
const RADIUS_MENU: u8 = 8;
/// ウィンドウの角丸
const RADIUS_WINDOW: u8 = 10;

/// ウィジェットの状態ごとの色（背景・枠線・文字）
struct WidgetColors {
    bg: Color32,
    stroke: Color32,
    text: Color32,
}

const NONINTERACTIVE: WidgetColors = WidgetColors {
    bg: Color32::from_rgb(0x2A, 0x2A, 0x2C),
    stroke: Color32::from_rgb(0x3A, 0x3A, 0x3D),
    text: TEXT_PRIMARY,
};
const INACTIVE: WidgetColors = WidgetColors {
    bg: Color32::from_rgb(0x33, 0x33, 0x37),
    stroke: Color32::from_rgb(0x44, 0x44, 0x48),
    text: TEXT_PRIMARY,
};
const HOVERED: WidgetColors = WidgetColors {
    bg: Color32::from_rgb(0x3F, 0x3F, 0x44),
    stroke: Color32::from_rgb(0x5A, 0x5A, 0x60),
    text: Color32::WHITE,
};
const ACTIVE: WidgetColors = WidgetColors {
    bg: ACCENT,
    stroke: Color32::from_rgb(0x4A, 0x8B, 0xD0),
    text: Color32::WHITE,
};

/// テーマを egui に適用する。起動時に一度だけ呼ぶ。
pub fn apply(ctx: &egui::Context) {
    // OS の設定にかかわらず、ダークテーマで固定する
    ctx.set_theme(ThemePreference::Dark);
    ctx.set_visuals_of(Theme::Dark, build_visuals());
    ctx.global_style_mut(apply_spacing);
}

fn build_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_PANEL;
    visuals.window_stroke = Stroke::new(1.0, NONINTERACTIVE.stroke);
    visuals.faint_bg_color = BG_FAINT;
    visuals.extreme_bg_color = BG_INPUT;
    visuals.text_edit_bg_color = Some(BG_INPUT);
    visuals.override_text_color = None;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = ERROR;
    visuals.hyperlink_color = Color32::from_rgb(0x6C, 0xB0, 0xF0);

    visuals.window_corner_radius = CornerRadius::same(RADIUS_WINDOW);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_MENU);

    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);

    set_widget(&mut visuals.widgets.noninteractive, &NONINTERACTIVE);
    set_widget(&mut visuals.widgets.inactive, &INACTIVE);
    set_widget(&mut visuals.widgets.hovered, &HOVERED);
    set_widget(&mut visuals.widgets.active, &ACTIVE);
    set_widget(&mut visuals.widgets.open, &INACTIVE);

    visuals
}

fn set_widget(widget: &mut WidgetVisuals, colors: &WidgetColors) {
    widget.bg_fill = colors.bg;
    widget.weak_bg_fill = colors.bg;
    widget.bg_stroke = Stroke::new(1.0, colors.stroke);
    widget.fg_stroke = Stroke::new(1.0, colors.text);
    widget.corner_radius = CornerRadius::same(RADIUS_WIDGET);
}

/// 余白を広げる。既定値（item_spacing 8x3、button_padding 4x1 など）から、おおむね 1.5〜2 倍にする
fn apply_spacing(style: &mut Style) {
    let spacing = &mut style.spacing;
    spacing.item_spacing = Vec2::new(10.0, 8.0);
    spacing.button_padding = Vec2::new(10.0, 5.0);
    spacing.interact_size = Vec2::new(40.0, 28.0);
    spacing.window_margin = Margin::same(12);
    spacing.menu_margin = Margin::same(8);
    spacing.indent = 20.0;
}

/// 2 色のコントラスト比（WCAG 2.x の相対輝度による）
#[cfg(test)]
fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    fn channel(value: u8) -> f64 {
        let c = f64::from(value) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    fn luminance(c: Color32) -> f64 {
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la > lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 通常の文字に求められる WCAG AA の基準
    const AA: f64 = 4.5;

    #[test]
    fn body_text_is_readable_on_backgrounds() {
        for bg in [BG_PANEL, BG_INPUT, BG_FAINT] {
            assert!(contrast_ratio(TEXT_PRIMARY, bg) >= AA, "bg = {bg:?}");
        }
    }

    #[test]
    fn weak_text_is_readable_on_panel() {
        assert!(contrast_ratio(TEXT_WEAK, BG_PANEL) >= AA);
    }

    #[test]
    fn status_colors_are_readable_on_panel() {
        for color in [WARNING, ERROR, SUCCESS] {
            assert!(contrast_ratio(color, BG_PANEL) >= AA, "color = {color:?}");
        }
    }

    #[test]
    fn widget_text_is_readable_on_each_state() {
        for colors in [&NONINTERACTIVE, &INACTIVE, &HOVERED, &ACTIVE] {
            assert!(contrast_ratio(colors.text, colors.bg) >= AA);
        }
    }

    #[test]
    fn panel_is_not_pure_black() {
        assert_ne!(BG_PANEL, Color32::BLACK);
    }

    #[test]
    fn spacing_is_wider_than_egui_default() {
        let default = Style::default().spacing;
        let mut style = Style::default();
        apply_spacing(&mut style);
        assert!(style.spacing.item_spacing.y > default.item_spacing.y);
        assert!(style.spacing.button_padding.x > default.button_padding.x);
        assert!(style.spacing.interact_size.y > default.interact_size.y);
    }
}
