use crate::engine::TranslationEngine;
use crate::error::Result;
use crate::package::ModelPackage;
use crate::sbd::RuleBasedSentenceSplitter;

/// 高レベルな翻訳パイプライン（文分割と推論の統合）
pub struct TranslatorPipeline {
    engine: TranslationEngine,
    split_sentences: bool,
    target_lang: String,
}

impl TranslatorPipeline {
    /// モデルパッケージからパイプラインを初期化する（デフォルトで文分割を有効化）
    pub fn new(package: &ModelPackage) -> Result<Self> {
        let target_lang = package.metadata().to_code.clone();
        let engine = TranslationEngine::load(package)?;
        Ok(Self {
            engine,
            split_sentences: true,
            target_lang,
        })
    }

    /// 文分割の有効/無効を設定する
    pub fn with_sentence_splitting(mut self, enabled: bool) -> Self {
        self.split_sentences = enabled;
        self
    }

    /// ターゲット言語を明示的に指定する（テストや上書き用）
    pub fn with_target_lang(mut self, target_lang: impl Into<String>) -> Self {
        self.target_lang = target_lang.into();
        self
    }

    /// ターゲット言語を取得する
    pub fn target_lang(&self) -> &str {
        &self.target_lang
    }

    /// テキストを翻訳する
    pub fn translate(&self, text: &str) -> Result<String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        if !self.split_sentences {
            return self.engine.translate(trimmed);
        }

        // 改行や段落の構造を保持するため、行ごとに文分割・再構築する
        let raw_lines: Vec<&str> = text.split('\n').collect();
        let mut line_sentences: Vec<Vec<String>> = Vec::with_capacity(raw_lines.len());
        let mut all_sentences: Vec<String> = Vec::new();

        for line in &raw_lines {
            let line_trimmed = line.trim();
            if line_trimmed.is_empty() {
                line_sentences.push(Vec::new());
            } else {
                let sents = RuleBasedSentenceSplitter::split(line_trimmed);
                all_sentences.extend(sents.clone());
                line_sentences.push(sents);
            }
        }

        if all_sentences.is_empty() {
            return Ok(String::new());
        }

        let sentence_refs: Vec<&str> = all_sentences.iter().map(|s| s.as_str()).collect();
        let translated_all = self.engine.translate_batch(&sentence_refs)?;

        let separator = if is_no_space_lang(&self.target_lang) {
            ""
        } else {
            " "
        };

        let mut trans_iter = translated_all.into_iter();
        let mut translated_lines = Vec::with_capacity(line_sentences.len());

        for sents in line_sentences {
            if sents.is_empty() {
                translated_lines.push(String::new());
            } else {
                let line_trans: Vec<String> = (0..sents.len())
                    .filter_map(|_| trans_iter.next())
                    .collect();
                translated_lines.push(line_trans.join(separator));
            }
        }

        Ok(translated_lines.join("\n"))
    }
}

/// 分かち書き（スペース区切り）を行わない言語（日本語・中国語など）かどうかを判定する
pub fn is_no_space_lang(lang: &str) -> bool {
    let lower = lang.trim().to_ascii_lowercase();
    lower == "ja"
        || lower.starts_with("ja_")
        || lower.starts_with("ja-")
        || lower == "zh"
        || lower.starts_with("zh_")
        || lower.starts_with("zh-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_no_space_lang() {
        assert!(is_no_space_lang("ja"));
        assert!(is_no_space_lang("JA"));
        assert!(is_no_space_lang("ja_JP"));
        assert!(is_no_space_lang("ja-jp"));
        assert!(is_no_space_lang("zh"));
        assert!(is_no_space_lang("zh_CN"));
        assert!(is_no_space_lang("zh-tw"));

        assert!(!is_no_space_lang("en"));
        assert!(!is_no_space_lang("es"));
        assert!(!is_no_space_lang("fr"));
        assert!(!is_no_space_lang("de"));
    }
}
