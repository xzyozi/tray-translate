//! 言語コードの扱い（選択肢の組み立てと表示名）。画面描画には依存しない。

/// 導入済みモデルの一覧と現在の設定値から、選択肢として利用可能な言語一覧（重複なし、昇順）を返す。
///
/// 導入済みモデルのペアキー（例: `"en_ja"`）から言語コードを取り出す。
/// もし現在の言語が導入済み一覧に含まれていなくても、設定中の値が消えないよう追加する。
pub fn collect_available_languages(installed: &[String], current: &str) -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();
    for pair in installed {
        if let Some((from, to)) = pair.split_once('_') {
            let from_trimmed = from.trim().to_ascii_lowercase();
            let to_trimmed = to.trim().to_ascii_lowercase();
            if !from_trimmed.is_empty() {
                set.insert(from_trimmed);
            }
            if !to_trimmed.is_empty() {
                set.insert(to_trimmed);
            }
        }
    }
    let cur_trimmed = current.trim().to_ascii_lowercase();
    if !cur_trimmed.is_empty() {
        set.insert(cur_trimmed);
    }
    set.into_iter().collect()
}

/// 言語コードを人間にわかりやすい表示名にする。
pub fn language_display_name(code: &str) -> String {
    let lower = code.trim().to_ascii_lowercase();
    match lower.as_str() {
        "ja" => "日本語 (ja)".to_string(),
        "en" => "英語 (en)".to_string(),
        "zh" => "中国語 (zh)".to_string(),
        "es" => "スペイン語 (es)".to_string(),
        "fr" => "フランス語 (fr)".to_string(),
        "de" => "ドイツ語 (de)".to_string(),
        "ko" => "韓国語 (ko)".to_string(),
        "it" => "イタリア語 (it)".to_string(),
        "ru" => "ロシア語 (ru)".to_string(),
        "pt" => "ポルトガル語 (pt)".to_string(),
        "" => "未選択".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tray_translate_app::models::required_packages;

    #[test]
    fn collect_available_languages_deduplicates_and_sorts() {
        let installed = vec![
            "en_ja".to_string(),
            "ja_en".to_string(),
            "es_en".to_string(),
        ];
        let langs = collect_available_languages(&installed, "en");
        assert_eq!(langs, vec!["en", "es", "ja"]);
    }

    #[test]
    fn collect_available_languages_preserves_current_even_if_not_installed() {
        let installed = vec!["en_ja".to_string()];
        let langs = collect_available_languages(&installed, "fr");
        assert_eq!(langs, vec!["en", "fr", "ja"]);
    }

    #[test]
    fn collect_available_languages_handles_empty_or_whitespace() {
        let installed: Vec<String> = vec![];
        let langs = collect_available_languages(&installed, "   ");
        assert!(langs.is_empty());
    }

    #[test]
    fn language_display_name_known_and_unknown() {
        assert_eq!(language_display_name("ja"), "日本語 (ja)");
        assert_eq!(language_display_name("EN"), "英語 (en)");
        assert_eq!(language_display_name("es"), "スペイン語 (es)");
        assert_eq!(language_display_name("xyz"), "xyz");
        assert_eq!(language_display_name(""), "未選択");
    }

    #[test]
    fn swap_availability_logic() {
        // en_ja のみある場合、ja -> en への逆方向入れ替えは不可
        let installed = vec!["en_ja".to_string()];
        let missing_reverse = required_packages(&installed, "ja", "en");
        assert!(!missing_reverse.is_empty());

        // en_ja と ja_en の両方がある場合、入れ替え可能
        let installed_both = vec!["en_ja".to_string(), "ja_en".to_string()];
        let missing_reverse_both = required_packages(&installed_both, "ja", "en");
        assert!(missing_reverse_both.is_empty());
    }
}
