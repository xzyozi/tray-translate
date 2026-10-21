use crate::error::Result;
use crate::package::ModelPackage;
use ct2rs::tokenizers::sentencepiece::Tokenizer as SpmTokenizer;
use ct2rs::{Config, TranslationOptions, Translator};
use tracing::info;

/// ct2rs が入力末尾に自動付与する EOS トークン
const EOS_TOKEN: &str = "</s>";

/// Argos Translate と同じトークナイズ結果にするためのラッパー。
///
/// ct2rs の SentencePiece トークナイザーは入力末尾に `</s>` を必ず付ける。
/// Argos は `sentencepiece.encode(..., out_type=str)` の結果をそのまま渡し、
/// EOS を付けない(Argos の en_ja モデルは `model/config.json` を持たず、
/// CTranslate2 側も EOS を自動付与しない)。出力を Python と揃えるため、
/// ここで末尾の `</s>` を取り除く。
struct ArgosTokenizer {
    inner: SpmTokenizer,
}

impl ct2rs::Tokenizer for ArgosTokenizer {
    fn encode(&self, input: &str) -> anyhow::Result<Vec<String>> {
        let mut tokens = ct2rs::Tokenizer::encode(&self.inner, input)?;
        if tokens.last().map(String::as_str) == Some(EOS_TOKEN) {
            tokens.pop();
        }
        Ok(tokens)
    }

    fn decode(&self, tokens: Vec<String>) -> anyhow::Result<String> {
        ct2rs::Tokenizer::decode(&self.inner, tokens)
    }
}

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
    translator: Translator<ArgosTokenizer>,
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

        let tokenizer = ArgosTokenizer {
            inner: SpmTokenizer::from_file(&spm_path, &spm_path)?,
        };
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
