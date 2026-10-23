//! 日本語フォントの探索と登録（ADR-006）。
//!
//! フォントはバイナリに同梱せず、OS にあるものを探して使う。
//! egui の既定フォントには日本語の字形が無く、そのままだと文字が四角（豆腐）になる。
//! 見つからなかった場合は、OS ごとの案内文（[`missing_font_message`]）を画面とログに出す。

use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 日本語を表示できるフォントの候補（上から順に探す）。OS ごとに分けて持つ。
///
/// 並べ方の方針:
/// - 見た目の読みやすさが良いゴシック体を先に置く
/// - `.ttc`（複数書体の集合）は先頭の書体（index 0）を使うため、先頭が日本語書体のものだけを置く
#[cfg(target_os = "windows")]
fn candidate_paths() -> Vec<PathBuf> {
    // `C:\Windows` 固定にせず、OS が示すシステムルートを優先する
    let root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("windir"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let fonts = root.join("Fonts");
    // 游ゴシック Medium / Regular、BIZ UDゴシック、メイリオ、MS ゴシックの順
    [
        "YuGothM.ttc",
        "YuGothR.ttc",
        "BIZ-UDGothicR.ttc",
        "meiryo.ttc",
        "msgothic.ttc",
    ]
    .iter()
    .map(|name| fonts.join(name))
    .collect()
}

#[cfg(target_os = "macos")]
fn candidate_paths() -> Vec<PathBuf> {
    [
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",
        "/System/Library/Fonts/Hiragino Sans W3.ttc",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn candidate_paths() -> Vec<PathBuf> {
    [
        // Noto Sans CJK（ディストリビューションごとに置き場所が違う）
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto/NotoSansCJK-Regular.ttc",
        // IPA ゴシック・Takao ゴシック
        "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf",
        "/usr/share/fonts/ipa-gothic/ipag.ttf",
        "/usr/share/fonts/truetype/takao-gothic/TakaoPGothic.ttf",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

/// 候補のうち、最初に読み込めたフォントのパスと中身を返す。
/// 読み込み処理は引数で差し替えられる（テストで実ファイルに依存しないため）。
fn find_first_readable<F>(paths: &[PathBuf], mut read: F) -> Option<(PathBuf, Vec<u8>)>
where
    F: FnMut(&Path) -> std::io::Result<Vec<u8>>,
{
    paths.iter().find_map(|path| match read(path) {
        // 空ファイルは壊れたフォントとして扱い、次の候補へ進む
        Ok(bytes) if !bytes.is_empty() => Some((path.clone(), bytes)),
        _ => None,
    })
}

/// 実ファイルの読み込み（`find_first_readable` に渡す）
fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

/// フォントが見つからなかったときの案内文。OS ごとに、入れるべきフォントを示す。
pub fn missing_font_message() -> String {
    let hint = if cfg!(target_os = "windows") {
        "「設定 > アプリ > オプション機能」から日本語の補助フォントを追加してください"
    } else if cfg!(target_os = "macos") {
        "ヒラギノ角ゴシックが見つかりません。システムのフォントを確認してください"
    } else {
        "Noto Sans CJK などの日本語フォントを導入してください（例: fonts-noto-cjk）"
    };
    format!("日本語フォントが見つかりません（日本語が四角で表示される場合があります）。{hint}")
}

/// 日本語フォントを探して egui に登録する。登録したフォントのパスを返す。
/// 見つからない場合は何も登録せず `None` を返す。
pub fn install_japanese_font(ctx: &egui::Context) -> Option<String> {
    let (path, bytes) = find_first_readable(&candidate_paths(), read_file)?;

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
    Some(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn candidates_are_not_empty_on_this_os() {
        assert!(!candidate_paths().is_empty());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_real_font_can_be_installed() {
        let ctx = egui::Context::default();
        let loaded = install_japanese_font(&ctx);
        assert!(
            loaded.is_some(),
            "Windows実機で日本語フォント（YuGothM等）が正しく見つかりeguiに登録できること"
        );
        let path_str = loaded.unwrap();
        eprintln!("Windows実機で読み込まれたフォント: {path_str}");
        assert!(
            path_str.ends_with(".ttc") || path_str.ends_with(".ttf"),
            "フォントファイルパスが取得できていること: {path_str}"
        );
    }

    #[test]
    fn candidates_have_no_duplicates() {
        let list = candidate_paths();
        let mut sorted = list.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(list.len(), sorted.len());
    }

    #[test]
    fn picks_first_readable_candidate() {
        let list = paths(&["a", "b", "c"]);
        let found = find_first_readable(&list, |p| {
            if p == Path::new("b") || p == Path::new("c") {
                Ok(vec![1, 2, 3])
            } else {
                Err(io::Error::from(io::ErrorKind::NotFound))
            }
        });
        assert_eq!(found, Some((PathBuf::from("b"), vec![1, 2, 3])));
    }

    #[test]
    fn skips_empty_files() {
        let list = paths(&["empty", "ok"]);
        let found = find_first_readable(&list, |p| {
            if p == Path::new("empty") {
                Ok(Vec::new())
            } else {
                Ok(vec![9])
            }
        });
        assert_eq!(found, Some((PathBuf::from("ok"), vec![9])));
    }

    #[test]
    fn returns_none_when_nothing_is_readable() {
        let list = paths(&["a", "b"]);
        let found = find_first_readable(&list, |_| Err(io::Error::from(io::ErrorKind::NotFound)));
        assert_eq!(found, None);
    }

    #[test]
    fn stops_reading_after_first_hit() {
        let list = paths(&["a", "b", "c"]);
        let mut calls = 0;
        let _ = find_first_readable(&list, |_| {
            calls += 1;
            Ok(vec![1])
        });
        assert_eq!(calls, 1);
    }

    #[test]
    fn missing_message_mentions_tofu_and_a_next_step() {
        let message = missing_font_message();
        assert!(message.contains("日本語フォントが見つかりません"));
        assert!(message.contains("四角"));
    }
}
