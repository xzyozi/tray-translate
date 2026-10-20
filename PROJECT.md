# PROJECT

## 目的

Argos Translate を参考に、Rust コアと常駐 GUI で構成するオフライン翻訳ツールを作る。

## 状態

初期セットアップ中。全体の進行は Issue #1(ロードマップ)で管理する。

## 実行環境の方針

- 検証は Linux および Windows で行う。Python 関連の実行は `uv` を使う。
- Rust は `rust-toolchain.toml` で固定する。`rustup` が未導入の環境では先に導入する。
- 対象 OS と環境非依存の方針は [ADR-002](docs/adr/002-target-platform-and-environment-policy.md) で定義（Windows/Linux、CPU専用、完全オフライン）。

## 主要文書

- 用語集: [CONTEXT.md](CONTEXT.md)
- ドキュメントガバナンス: [docs/README.md](docs/README.md)
- 対象プラットフォーム方針: [ADR-002](docs/adr/002-target-platform-and-environment-policy.md)
