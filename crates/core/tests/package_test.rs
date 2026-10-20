use std::path::PathBuf;
use tempfile::tempdir;
use tray_translate_core::install_from_archive;

#[test]
fn test_install_from_argosmodel_archive() {
    let archive_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../.local/cache/argos-translate/downloads/translate-en_ja.argosmodel");

    if !archive_path.exists() {
        eprintln!(
            "Archive not found, skipping integration test: {}",
            archive_path.display()
        );
        return;
    }

    let temp_dir = tempdir().expect("failed to create temp dir");
    let package = install_from_archive(&archive_path, temp_dir.path())
        .expect("failed to install from archive");

    assert_eq!(package.metadata().from_code, "en");
    assert_eq!(package.metadata().to_code, "ja");
    assert!(package.sentencepiece_path().exists());
    assert!(package.model_dir().exists());
}
