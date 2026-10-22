use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use tracing::info;
use zip::ZipArchive;

/// 展開時の上限設定（Zip bomb / リソース枯渇対策）
const MAX_ARCHIVE_ENTRIES: usize = 1_000;
const MAX_UNCOMPRESSED_SIZE: u64 = 2 * 1024 * 1024 * 1024; // 2 GB

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

/// `.argosmodel` (ZIP アーカイブ) を指定の宛先ディレクトリへ安全・原子的に展開・インストールする。
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

    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(CoreError::InvalidPackage(format!(
            "Archive contains too many entries ({} > limit {})",
            archive.len(),
            MAX_ARCHIVE_ENTRIES
        )));
    }

    // 展開先ベースディレクトリを確保
    fs::create_dir_all(target_dir)?;

    // 一時ディレクトリを作成して安全に展開（途中失敗で壊れたファイルを残さない）
    let temp_dir = tempfile::Builder::new()
        .prefix(".tmp_pkg_")
        .tempdir_in(target_dir)?;
    let staging_root = temp_dir.path();

    // アーカイブ内のトップディレクトリ（例: "en_ja/"）を検出
    let top_prefix = if archive.len() > 0 {
        let first_name = archive.by_index(0)?.name().to_string();
        if first_name.contains('/') {
            first_name.split('/').next().unwrap_or_default().to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mut total_uncompressed_bytes: u64 = 0;

    for i in 0..archive.len() {
        let mut zip_file = archive.by_index(i)?;

        // Zip Slip 対策: enclosed_name によるパストラバーサル防止検証
        let enclosed_path = zip_file.enclosed_name().ok_or_else(|| {
            CoreError::InvalidPackage(format!(
                "Insecure entry path in archive (Zip Slip attempt): {}",
                zip_file.name()
            ))
        })?;

        // トッププレフィックスを剥がして相対パスにする
        let relative_path = if !top_prefix.is_empty() {
            match enclosed_path.strip_prefix(&top_prefix) {
                Ok(stripped) => stripped.to_path_buf(),
                Err(_) => enclosed_path.to_path_buf(),
            }
        } else {
            enclosed_path.to_path_buf()
        };

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let out_path = staging_root.join(&relative_path);

        if zip_file.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
            }

            total_uncompressed_bytes += zip_file.size();
            if total_uncompressed_bytes > MAX_UNCOMPRESSED_SIZE {
                return Err(CoreError::InvalidPackage(format!(
                    "Uncompressed size exceeds limit ({} bytes > {} limit)",
                    total_uncompressed_bytes, MAX_UNCOMPRESSED_SIZE
                )));
            }

            let mut out_file = File::create(&out_path)?;
            io::copy(&mut zip_file, &mut out_file)?;
        }
    }

    // 展開されたパッケージの構造・メタデータを検証
    let metadata = PackageMetadata::load_from_file(staging_root.join("metadata.json"))?;
    let destination_name = if !top_prefix.is_empty() {
        top_prefix
    } else {
        format!("{}_{}", metadata.from_code, metadata.to_code)
    };
    let destination = target_dir.join(destination_name);

    // 既存のインストールがあれば置換
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }

    // 原子的リネームで確定（Windows では target_dir 同一ドライブ内の一時ディレクトリなので高速 rename 可能）
    fs::rename(staging_root, &destination)?;

    ModelPackage::open(&destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn test_parse_metadata() {
        let json = r#"{
            "package_version": "1.0",
            "argos_version": "1.0",
            "from_code": "en",
            "from_name": "English",
            "to_code": "ja",
            "to_name": "Japanese"
        }"#;

        let meta: PackageMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.from_code, "en");
        assert_eq!(meta.to_code, "ja");
    }

    #[test]
    fn test_reject_zip_slip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let malicious_zip_path = temp_dir.path().join("malicious.zip");

        // パストラバーサルを含む zip を作成
        {
            let file = File::create(&malicious_zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = SimpleFileOptions::default();

            zip.start_file("../evil.txt", options).unwrap();
            zip.write_all(b"evil content").unwrap();
            zip.finish().unwrap();
        }

        let install_dest = temp_dir.path().join("installed");
        let result = install_from_archive(&malicious_zip_path, &install_dest);

        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Insecure entry path") || err_msg.contains("Zip Slip"));
        assert!(!temp_dir.path().join("evil.txt").exists());
    }
}
