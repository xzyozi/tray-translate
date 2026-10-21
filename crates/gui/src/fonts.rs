use eframe::egui;
use std::sync::Arc;

/// 日本語を表示できるフォントの候補（上から順に探す）。
/// egui の既定フォントには日本語の字形が無く、そのままだと文字が四角になる。
const CANDIDATES: &[&str] = &[
    // Windows
    r"C:\Windows\Fonts\YuGothM.ttc",
    r"C:\Windows\Fonts\meiryo.ttc",
    r"C:\Windows\Fonts\msgothic.ttc",
    // Linux
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    // macOS
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
];

/// 日本語フォントを探して egui に登録する。登録したフォントのパスを返す。
/// 見つからない場合は何も登録せず `None` を返す。
pub fn install_japanese_font(ctx: &egui::Context) -> Option<String> {
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "japanese".to_owned(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
        // 既定フォントの後ろに足し、英数字は既定のまま、日本語だけこのフォントで描く
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(list) = fonts.families.get_mut(&family) {
                list.push("japanese".to_owned());
            }
        }
        ctx.set_fonts(fonts);
        return Some((*path).to_string());
    }
    None
}
