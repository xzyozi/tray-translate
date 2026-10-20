use crate::engine::TranslationEngine;
use crate::error::Result;
use crate::package::ModelPackage;
use crate::sbd::RuleBasedSentenceSplitter;

/// 高レベルな翻訳パイプライン（文分割と推論の統合）
pub struct TranslatorPipeline {
    engine: TranslationEngine,
    split_sentences: bool,
}

impl TranslatorPipeline {
    /// モデルパッケージからパイプラインを初期化する（デフォルトで文分割を有効化）
    pub fn new(package: &ModelPackage) -> Result<Self> {
        let engine = TranslationEngine::load(package)?;
        Ok(Self {
            engine,
            split_sentences: true,
        })
    }

    /// 文分割の有効/無効を設定する
    pub fn with_sentence_splitting(mut self, enabled: bool) -> Self {
        self.split_sentences = enabled;
        self
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

        let sentences = RuleBasedSentenceSplitter::split(trimmed);
        if sentences.is_empty() {
            return Ok(String::new());
        }

        let sentence_refs: Vec<&str> = sentences.iter().map(|s| s.as_str()).collect();
        let translated_sentences = self.engine.translate_batch(&sentence_refs)?;

        // 言語に応じた自然な結合（必要に応じて空白または直接結合）
        let result = translated_sentences.join(" ");
        Ok(result)
    }
}
