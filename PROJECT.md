# PROJECT

## 目的

Argos Translate を参考に、Rust コアと常駐 GUI で構成するオフライン翻訳ツールを作る。

## 状態

v0.1.0 公開済み。基盤の安定性・安全性強化、および GUI の UI/UX 刷新フェーズ。全体の進行は Issue #1(ロードマップ) および各機能 Issue で管理する。

## 実行環境の方針

- 検証は Linux および Windows で行う。Python 関連の実行は `uv` を使う。
- Rust は `rust-toolchain.toml` で固定する。`rustup` が未導入の環境では先に導入する。
- 対象 OS と環境非依存の方針は [ADR-002](docs/adr/002-target-platform-and-environment-policy.md) で定義（Windows/Linux、CPU専用、完全オフライン）。

## 主要文書

- 用語集: [CONTEXT.md](CONTEXT.md)
- ドキュメントガバナンス: [docs/README.md](docs/README.md)
- 対象プラットフォーム方針: [ADR-002](docs/adr/002-target-platform-and-environment-policy.md)
- GUI 技術選定: [docs/adr/004-gui-technology-selection.md](docs/adr/004-gui-technology-selection.md)
- panic 方針と言語コードの前提: [ADR-005](docs/adr/005-panic-policy.md)
- 実モデルを使った確認手順（CLI）: [docs/test/real-model-verification.md](docs/test/real-model-verification.md)
- GUI 手動動作確認手順: [docs/test/gui-manual-test-guide.md](docs/test/gui-manual-test-guide.md)
