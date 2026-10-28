---
title: "実モデルを使った確認手順（CLI・画面なし）"
document_type: "operation_procedure"
document_role: "procedure"
ssot_scope: "test_evidence"
status: "review"
version: "1.0"
created_at: "2026-10-23"
updated_at: "2026-10-27"
author: "xzyozi"
owner: "xzyozi"
purpose: "画面のない Linux 検証機で、実モデルによる翻訳と Argos との出力比較を確認する手順を定義するため"
related_documents:
  - "ドキュメントガバナンス（相対パス: ../README.md）"
  - "ADR-003 G3 の合格基準（相対パス: ../adr/003-g3-acceptance-criteria.md）"
  - "テスト文セット（相対パス: ../testdata/sentences.json）"
---

# 実モデルを使った確認手順（CLI・画面なし）

画面のない Linux 検証機でも実行できる確認手順。コードの変更後、`cargo test` では確認できない「実モデルでの翻訳」を確かめる。

## 前提

- モデルの保存先は `TT_MODEL_DIR`（未指定時は OS 標準のデータディレクトリ）。
- 初回のみ、モデルのダウンロードで通信が発生する（プロキシは `HTTPS_PROXY` / `HTTP_PROXY`）。

## 手順

```bash
# 1. ビルド（CI と同じ検証）
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked

# 2. モデルの導入（初回のみ）
cargo run --release --bin tt -- download en ja

# 3. 導入済みモデルの確認
cargo run --release --bin tt -- list

# 4. 翻訳（引数）
cargo run --release --bin tt -- translate en_ja "Rust is amazing. Offline translation works perfectly."

# 5. 翻訳（標準入力）
echo "Hello world" | cargo run --release --bin tt -- translate en_ja -
```

## 期待結果

- 手順 4・5 で日本語の訳文が出力され、終了コードが 0 である。
- 手順 3 の一覧に `en_ja` が含まれる。

## G3（Argos との出力一致）の比較

比較の基準と合格基準は [ADR-003](../adr/003-g3-acceptance-criteria.md) を参照する。

```bash
TT_MODEL_DIR=<モデルの保存先> cargo run --release --bin tt -- bench en_ja docs/testdata/sentences.json --compare <python の bench JSON>
```
