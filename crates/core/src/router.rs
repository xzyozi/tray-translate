use crate::error::{CoreError, Result};
use crate::package::ModelPackage;
use crate::paths::get_model_dir;
use crate::pipeline::TranslatorPipeline;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// 翻訳のルーティング経路
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationRoute {
    /// 入力言語と出力言語が同一（翻訳不要）
    Identity,
    /// 直接ペアが存在（例: en -> ja）
    Direct { pair_key: String },
    /// 英語（en）を経由するピボット翻訳（例: ja -> en -> es）
    Pivot {
        step1_key: String,
        step2_key: String,
    },
}

/// 複数言語モデルの管理およびピボット（英語中継）ルーティングを担うルーター
pub struct TranslationRouter {
    packages_dir: PathBuf,
    available_pairs: HashMap<String, PathBuf>,
    pipeline_cache: HashMap<String, TranslatorPipeline>,
}

impl TranslationRouter {
    /// デフォルトのモデル保存ディレクトリ（`get_model_dir`）からルーターを初期化する
    pub fn new() -> Result<Self> {
        let dir = get_model_dir()?;
        Self::with_packages_dir(dir)
    }

    /// 指定したディレクトリから利用可能なパッケージをスキャンして初期化する
    pub fn with_packages_dir(packages_dir: impl AsRef<Path>) -> Result<Self> {
        let packages_dir = packages_dir.as_ref().to_path_buf();
        let mut router = Self {
            packages_dir,
            available_pairs: HashMap::new(),
            pipeline_cache: HashMap::new(),
        };
        router.refresh_available_packages()?;
        Ok(router)
    }

    /// パッケージディレクトリをスキャンし、利用可能な言語ペア一覧を更新する
    pub fn refresh_available_packages(&mut self) -> Result<()> {
        self.available_pairs.clear();
        if !self.packages_dir.exists() {
            return Ok(());
        }

        // read_dir の順序は OS 依存のため、パス順に並べて重複時の採用(後勝ち)を決定的にする
        let mut paths = std::fs::read_dir(&self.packages_dir)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.sort();

        for path in paths {
            if path.is_dir() {
                match ModelPackage::open(&path) {
                    Ok(package) => {
                        let key = format!(
                            "{}_{}",
                            package.metadata().from_code,
                            package.metadata().to_code
                        );
                        if let Some(old_path) =
                            self.available_pairs.insert(key.clone(), path.clone())
                        {
                            warn!(
                                pair = %key,
                                previous_path = %old_path.display(),
                                current_path = %path.display(),
                                "Duplicate model package detected for pair; overwritten with latest entry"
                            );
                        }
                    }
                    Err(err) => {
                        warn!(
                            path = %path.display(),
                            error = %err,
                            "Failed to open model package directory, skipping"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// 明示的にパッケージを登録する
    pub fn register_package(&mut self, package: &ModelPackage) {
        let key = format!(
            "{}_{}",
            package.metadata().from_code,
            package.metadata().to_code
        );
        if let Some(old_path) = self
            .available_pairs
            .insert(key.clone(), package.root_dir().to_path_buf())
        {
            warn!(
                pair = %key,
                previous_path = %old_path.display(),
                current_path = %package.root_dir().display(),
                "Duplicate model package detected during manual registration; overwritten"
            );
        }
    }

    /// 指定された言語ペアのルーティング経路を決定する
    pub fn plan_route(&self, from: &str, to: &str) -> Result<TranslationRoute> {
        let from = from.to_ascii_lowercase();
        let to = to.to_ascii_lowercase();

        if from == to {
            return Ok(TranslationRoute::Identity);
        }

        let direct_key = format!("{from}_{to}");
        if self.available_pairs.contains_key(&direct_key) {
            return Ok(TranslationRoute::Direct {
                pair_key: direct_key,
            });
        }

        // 英語を経由するピボット翻訳の可能性を探索 (from -> en -> to)
        if from != "en" && to != "en" {
            let step1_key = format!("{from}_en");
            let step2_key = format!("en_{to}");

            if self.available_pairs.contains_key(&step1_key)
                && self.available_pairs.contains_key(&step2_key)
            {
                return Ok(TranslationRoute::Pivot {
                    step1_key,
                    step2_key,
                });
            }
        }

        Err(CoreError::UnsupportedLanguagePair(from, to))
    }

    /// 指定した言語ペアキーのパイプラインを取得（キャッシュになければロード）
    fn get_or_load_pipeline(&mut self, pair_key: &str) -> Result<&TranslatorPipeline> {
        if !self.pipeline_cache.contains_key(pair_key) {
            let path = self
                .available_pairs
                .get(pair_key)
                .ok_or_else(|| CoreError::UnknownPairKey(pair_key.to_string()))?;

            info!(pair = %pair_key, path = %path.display(), "Loading pipeline into cache");
            let package = ModelPackage::open(path)?;
            let pipeline = TranslatorPipeline::new(&package)?;
            self.pipeline_cache.insert(pair_key.to_string(), pipeline);
        }

        Ok(self.pipeline_cache.get(pair_key).unwrap())
    }

    /// ルーティングに従ってテキストを翻訳する
    pub fn translate(&mut self, from: &str, to: &str, text: &str) -> Result<String> {
        let route = self.plan_route(from, to)?;
        match route {
            TranslationRoute::Identity => Ok(text.to_string()),
            TranslationRoute::Direct { pair_key } => {
                let pipeline = self.get_or_load_pipeline(&pair_key)?;
                pipeline.translate(text)
            }
            TranslationRoute::Pivot {
                step1_key,
                step2_key,
            } => {
                info!(
                    step1 = %step1_key,
                    step2 = %step2_key,
                    "Executing 2-step pivot translation via 'en'"
                );
                let intermediate = {
                    let pipeline1 = self.get_or_load_pipeline(&step1_key)?;
                    pipeline1.translate(text)?
                };

                let pipeline2 = self.get_or_load_pipeline(&step2_key)?;
                pipeline2.translate(&intermediate)
            }
        }
    }

    /// 現在ロードされているパイプライン数（メモリ消費の目安）
    pub fn loaded_count(&self) -> usize {
        self.pipeline_cache.len()
    }

    /// キャッシュしているパイプラインをすべてアンロード（破棄）してメモリを解放する
    pub fn unload_all(&mut self) {
        if !self.pipeline_cache.is_empty() {
            info!(
                count = self.pipeline_cache.len(),
                "Unloading all cached translation pipelines"
            );
            self.pipeline_cache.clear();
        }
    }

    /// 利用可能な言語ペアの一覧（ソート済み）
    pub fn available_pairs(&self) -> Vec<String> {
        let mut pairs: Vec<String> = self.available_pairs.keys().cloned().collect();
        pairs.sort();
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_route() {
        let mut router = TranslationRouter {
            packages_dir: PathBuf::from("dummy"),
            available_pairs: HashMap::new(),
            pipeline_cache: HashMap::new(),
        };

        router
            .available_pairs
            .insert("en_ja".to_string(), PathBuf::from("en_ja"));
        router
            .available_pairs
            .insert("ja_en".to_string(), PathBuf::from("ja_en"));
        router
            .available_pairs
            .insert("en_es".to_string(), PathBuf::from("en_es"));

        // 同一言語
        assert_eq!(
            router.plan_route("ja", "ja").unwrap(),
            TranslationRoute::Identity
        );

        // 直接ペア
        assert_eq!(
            router.plan_route("en", "ja").unwrap(),
            TranslationRoute::Direct {
                pair_key: "en_ja".to_string()
            }
        );

        // ピボットペア (ja -> en -> es)
        assert_eq!(
            router.plan_route("ja", "es").unwrap(),
            TranslationRoute::Pivot {
                step1_key: "ja_en".to_string(),
                step2_key: "en_es".to_string()
            }
        );

        // 非対応ペア
        assert!(router.plan_route("ja", "fr").is_err());
    }

    #[test]
    fn test_unknown_pair_key_has_dedicated_error() {
        let mut router = TranslationRouter {
            packages_dir: PathBuf::from("dummy"),
            available_pairs: HashMap::new(),
            pipeline_cache: HashMap::new(),
        };

        let result = router.get_or_load_pipeline("not_a_known_pair");
        assert!(matches!(
            result,
            Err(CoreError::UnknownPairKey(key)) if key == "not_a_known_pair"
        ));
    }

    #[test]
    fn test_refresh_skips_invalid_package_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        // 壊れた（空の）ディレクトリを作成
        std::fs::create_dir_all(temp_dir.path().join("broken_model")).unwrap();

        let mut router = TranslationRouter {
            packages_dir: temp_dir.path().to_path_buf(),
            available_pairs: HashMap::new(),
            pipeline_cache: HashMap::new(),
        };

        // エラーでパニックや中断せず、警告ログを出して空のリストで完了すること
        assert!(router.refresh_available_packages().is_ok());
        assert!(router.available_pairs().is_empty());
    }
}
