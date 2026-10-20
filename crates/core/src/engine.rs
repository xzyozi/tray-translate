use crate::error::Result;
use crate::package::ModelPackage;
use ct2rs::tokenizers::sentencepiece::Tokenizer;
use ct2rs::{Config, TranslationOptions, Translator};
use tracing::info;

/// 翻訳推論のパラメータ設定
#[derive(Debug, Clone)]
pub struct TranslationConfig {
    pub beam_size: usize,
    pub length_penalty: f32,
    pub replace_unknowns: bool,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        // Argos Translate のデフォルトパラメータに準拠
        Self {
            beam_size: 4,
            length_penalty: 0.2,
            replace_unknowns: true,
        }
    }
}

/// CTranslate2 と SentencePiece トークナイザーをラップした翻訳エンジン
pub struct TranslationEngine {
    translator: Translator<Tokenizer>,
    config: TranslationConfig,
}

impl TranslationEngine {
    /// モデルパッケージから翻訳エンジンをロードする
    pub fn load(package: &ModelPackage) -> Result<Self> {
        Self::load_with_config(package, TranslationConfig::default())
    }

    /// 設定を指定してモデルパッケージから翻訳エンジンをロードする
    pub fn load_with_config(package: &ModelPackage, config: TranslationConfig) -> Result<Self> {
        let model_dir = package.model_dir();
        let spm_path = package.sentencepiece_path();

        info!(
            model = %model_dir.display(),
            spm = %spm_path.display(),
            "Loading translation engine"
        );

        let tokenizer = Tokenizer::from_file(&spm_path, &spm_path)?;
        let translator = Translator::with_tokenizer(model_dir, tokenizer, &Config::default())?;

        Ok(Self { translator, config })
    }

    /// 単一テキストを翻訳する
    pub fn translate(&self, text: &str) -> Result<String> {
        let results = self.translate_batch(&[text])?;
        Ok(results.into_iter().next().unwrap_or_default())
    }

    /// 複数テキストを一括でバッチ翻訳する
    pub fn translate_batch(&self, texts: &[&str]) -> Result<Vec<String>> {
        let options = TranslationOptions {
            beam_size: self.config.beam_size,
            length_penalty: self.config.length_penalty,
            replace_unknowns: self.config.replace_unknowns,
            ..Default::default()
        };

        let raw_results = self.translator.translate_batch(texts, &options, None)?;
        let outputs = raw_results.into_iter().map(|(text, _)| text).collect();

        Ok(outputs)
    }
}
