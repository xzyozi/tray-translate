use crate::error::{CoreError, Result};
use crate::package::{install_from_archive, ModelPackage};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::path::Path;
use tracing::info;

pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/argosopentech/argospm-index/main/index.json";

/// ダウンロード用の HTTP エージェントを作る。
///
/// - 環境変数のプロキシ設定（`HTTPS_PROXY` / `HTTP_PROXY` など）を使う（`proxy-from-env`）。
/// - TLS の信頼するルート証明書は、同梱の Mozilla ルートではなく OS の証明書ストアを使う
///   （`native-certs`）。社内 CA が TLS を中継するプロキシ環境でも、OS に CA が入っていれば接続できる。
/// - 証明書の検証は無効化しない。
fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new().try_proxy_from_env(true).build()
}

/// パッケージインデックスの各エントリ
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageIndexEntry {
    pub package_version: String,
    pub argos_version: String,
    pub from_code: String,
    pub from_name: String,
    pub to_code: String,
    pub to_name: String,
    #[serde(default)]
    pub links: Vec<String>,
}

impl PackageIndexEntry {
    /// 利用可能な HTTPS ダウンロード URL を取得する
    pub fn download_url(&self) -> Result<&str> {
        self.links
            .iter()
            .find(|link| link.starts_with("https://") || link.starts_with("http://"))
            .map(|s| s.as_str())
            .ok_or(CoreError::NoDownloadLink)
    }
}

/// 公式モデルインデックスの取得およびダウンロードを担うハブ
pub struct ModelHub;

impl ModelHub {
    /// デフォルトのインデックス URL から言語モデル一覧を取得する
    pub fn fetch_index() -> Result<Vec<PackageIndexEntry>> {
        Self::fetch_index_from_url(DEFAULT_INDEX_URL)
    }

    /// 指定した URL からインデックスを取得する
    pub fn fetch_index_from_url(url: &str) -> Result<Vec<PackageIndexEntry>> {
        info!(url = %url, "Fetching model package index");
        let response = http_agent().get(url).call()?;
        let entries: Vec<PackageIndexEntry> = response.into_json()?;
        Ok(entries)
    }

    /// インデックスから指定された言語ペア（from -> to）のエントリを検索する
    pub fn find_package(
        entries: &[PackageIndexEntry],
        from: &str,
        to: &str,
    ) -> Result<PackageIndexEntry> {
        entries
            .iter()
            .find(|e| e.from_code.eq_ignore_ascii_case(from) && e.to_code.eq_ignore_ascii_case(to))
            .cloned()
            .ok_or_else(|| CoreError::PackageNotFound(from.to_string(), to.to_string()))
    }

    /// パッケージをダウンロードし、ターゲットディレクトリへ展開・インストールする
    pub fn download_and_install(
        entry: &PackageIndexEntry,
        target_dir: impl AsRef<Path>,
    ) -> Result<ModelPackage> {
        let url = entry.download_url()?;
        let target_dir = target_dir.as_ref();

        info!(
            from = %entry.from_code,
            to = %entry.to_code,
            url = %url,
            target = %target_dir.display(),
            "Downloading model package"
        );

        let response = http_agent().get(url).call()?;

        // 一時ファイルに保存
        let temp_dir = tempfile::tempdir()?;
        let temp_archive_path = temp_dir.path().join("model.argosmodel");
        {
            let mut dest = File::create(&temp_archive_path)?;
            let mut reader = response.into_reader();
            io::copy(&mut reader, &mut dest)?;
        }

        info!(
            path = %temp_archive_path.display(),
            "Download completed. Extracting archive"
        );

        let package = install_from_archive(&temp_archive_path, target_dir)?;
        Ok(package)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_index_entry() {
        let json = r#"[
            {
                "package_version": "1.1",
                "argos_version": "1.1",
                "from_code": "en",
                "from_name": "English",
                "to_code": "ja",
                "to_name": "Japanese",
                "links": [
                    "https://argos-net.com/v1/translate-en_ja-1_1.argosmodel",
                    "ipfs://QmQjHs8ZhCU6WTV9RRHQMXgy4cDwyjvLUZ4bBp6J9GMCRb"
                ]
            }
        ]"#;

        let entries: Vec<PackageIndexEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.from_code, "en");
        assert_eq!(entry.to_code, "ja");
        assert_eq!(
            entry.download_url().unwrap(),
            "https://argos-net.com/v1/translate-en_ja-1_1.argosmodel"
        );

        let found = ModelHub::find_package(&entries, "EN", "JA").unwrap();
        assert_eq!(found.from_code, "en");

        let not_found = ModelHub::find_package(&entries, "en", "es");
        assert!(not_found.is_err());
    }
}
