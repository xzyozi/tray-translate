use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use tracing::info;
use zip::ZipArchive;

/// Argos 形式の言語モデルパッケージの metadata.json 構造体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageMetadata {
    pub package_version: String,
    pub argos_version: String,
    pub from_code: String,
    pub from_name: String,
    pub to_code: String,
    pub to_name: String,
}

impl PackageMetadata {
    /// 指定されたパスの metadata.json を読み込んでパースする
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content =
            fs::read_to_string(path).map_err(|_| CoreError::InvalidMetadata(path.to_path_buf()))?;
        let meta: Self = serde_json::from_str(&content)
            .map_err(|_| CoreError::InvalidMetadata(path.to_path_buf()))?;
        Ok(meta)
    }
}

/// 展開済みの言語モデルパッケージを表す構造体
#[derive(Debug, Clone)]
pub struct ModelPackage {
    root_dir: PathBuf,
    metadata: PackageMetadata,
}

impl ModelPackage {
    /// 展開済みパッケージディレクトリを検証し、インスタンスを生成する
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root_dir = path.as_ref().to_path_buf();
        let meta_path = root_dir.join("metadata.json");
        let metadata = PackageMetadata::load_from_file(&meta_path)?;

        let spm_path = root_dir.join("sentencepiece.model");
        if !spm_path.exists() {
            return Err(CoreError::MissingModelFile(spm_path));
        }

        let model_dir = root_dir.join("model");
        if !model_dir.exists() {
            return Err(CoreError::MissingModelFile(model_dir));
        }

        Ok(Self { root_dir, metadata })
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn metadata(&self) -> &PackageMetadata {
        &self.metadata
    }

    pub fn model_dir(&self) -> PathBuf {
        self.root_dir.join("model")
    }

    pub fn sentencepiece_path(&self) -> PathBuf {
        self.root_dir.join("sentencepiece.model")
    }

    pub fn pair_key(&self) -> String {
        format!("{}_{}", self.metadata.from_code, self.metadata.to_code)
    }
}

/// `.argosmodel` (ZIP アーカイブ) を指定の宛先ディレクトリへ展開・インストールする。
pub fn install_from_archive(
    archive_path: impl AsRef<Path>,
    target_dir: impl AsRef<Path>,
) -> Result<ModelPackage> {
    let archive_path = archive_path.as_ref();
    let target_dir = target_dir.as_ref();

    info!(
        archive = %archive_path.display(),
        target = %target_dir.display(),
        "Installing model package from archive"
    );

    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;

    // アーカイブ内のトップディレクトリ（例: "en_ja/"）を検出する
    let top_prefix = {
        let first_name = archive.by_index(0)?.name().to_string();
        first_name.split('/').next().unwrap_or_default().to_string()
    };

    let destination = if !top_prefix.is_empty() {
        target_dir.join(&top_prefix)
    } else {
        target_dir.to_path_buf()
    };

    fs::create_dir_all(&destination)?;

    for i in 0..archive.len() {
        let mut zip_file = archive.by_index(i)?;
        let name = zip_file.name();

        // 不要な上位ディレクトリプレフィックスを剥がして相対パスにする
        let relative_path = if !top_prefix.is_empty() && name.starts_with(&top_prefix) {
            name.strip_prefix(&top_prefix)
                .unwrap_or(name)
                .trim_start_matches('/')
        } else {
            name
        };

        if relative_path.is_empty() {
            continue;
        }

        let out_path = destination.join(relative_path);

        if zip_file.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
            }
            let mut out_file = File::create(&out_path)?;
            io::copy(&mut zip_file, &mut out_file)?;
        }
    }

    ModelPackage::open(&destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_metadata() {
        let json = r#"{
            "package_version": "1.1",
            "argos_version": "1.1",
            "from_code": "en",
            "from_name": "English",
            "to_code": "ja",
            "to_name": "Japanese"
        }"#;

        let meta: PackageMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.from_code, "en");
        assert_eq!(meta.to_code, "ja");
        assert_eq!(meta.package_version, "1.1");
    }
}
