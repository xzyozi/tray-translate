---
title: "対象プラットフォームおよび環境非依存方針"
document_type: "architecture_decision_record"
document_number: "ADR-002"
document_role: "rationale"
ssot_scope: "decision_rationale"
status: "accepted"
version: "1.1"
created_at: "2026-10-20"
updated_at: "2026-10-27"
author: "tray-translate team"
owner: "tray-translate team"
purpose: "対象OS・CPUアーキテクチャ、ビルド基盤、および環境非依存方針を決定するため"
related_documents:
  - "ドキュメントガバナンス（相対パス: ../README.md）"
  - "用語集（相対パス: ../../CONTEXT.md）"
  - "プロジェクト方針（相対パス: ../../PROJECT.md）"
---

# ADR-002: 対象プラットフォームおよび環境非依存方針

| 項目           | 内容                |
| :------------- | :------------------ |
| ステータス     | accepted            |
| 決定日         | 2026-10-20          |
| 決定者・承認者 | tray-translate team |
| 関連Issue      | #2, #5, #8          |

## 1. 背景・課題

tray-translate は、常駐型の軽量オフライン翻訳ツールとして開発を進めている。開発を効率的かつ移植性高く進めるため、サポート対象とする OS、CPU アーキテクチャ、推論実行デバイス、ライブラリのリンク方針、および設定・データ保存の標準仕様を早期に確定する必要があった。

また、検証フェーズ（Issue #4、#5）において以下の知見が得られた：
- Linux 検証機（Rocky Linux 8.10 / VirtualBox）では AVX/AVX2 が無効化されており、CTranslate2 は `GENERIC` フォールバックで動作した。
- Windows 物理機（Intel Core i5-12400F）では AVX2 が正常に認識され、推論速度が約10〜13倍に向上した。
- Windows MSVC ビルド時、CTranslate2 の CMake ビルドが静的ランタイム（`/MT`）を用いるため、Rust 側にも `+crt-static` を強制する必要がある。
- Linux 環境における glibc のバージョン互換性問題（Ubuntu 等の新しい環境でビルドすると古い glibc 環境で動作しない）。

## 2. 決定

### 2.1 対象 OS および CPU アーキテクチャ

| 優先度              | OS                                 | CPU                       | 備考                                          |
| :------------------ | :--------------------------------- | :------------------------ | :-------------------------------------------- |
| **Tier 1 (最優先)** | **Windows 10 / 11**                | **x86_64**                | MSVC ツールチェーン、`+crt-static` 静的リンク |
| **Tier 1 (最優先)** | **Linux** (RHEL 8系/Rocky 8系互換) | **x86_64**                | glibc 2.28 基準ビルド                         |
| **Tier 2**          | **macOS** (12 以降)                | **arm64** (Apple Silicon) | egui/eframe GUI 提供時に検証                  |
| **Tier 2**          | **Linux**                          | **arm64** (aarch64)       | 将来のクロスコンパイル対象                    |

- **Linux ビルド基盤**: Linux 向けバイナリの配布用ビルドは、Rocky Linux 8 系（glibc 2.28 基準）のコンテナまたは環境で行う。これにより、より新しい glibc を持つ Ubuntu や Fedora 等でも透過的に動作可能とする。

### 2.2 推論実行デバイス（CPU 専任・GPU 非使用）

- **推論は CPU のみで実行する**（GPU / CUDA / ROCm は使用しない）。
  - **理由**: 常駐ツールとしての軽快さ、GPU メモリの専有回避、ドライバ依存の排除、配布バイナリの肥大化防止。
- **命令セットの活用**:
  - CPU の AVX2 / AVX / FMA 命令セットが存在する場合は CTranslate2 のランタイムディスパッチにより自動活用する。
  - AVX を持たない環境（仮想マシンや古い CPU）でも `GENERIC` / `ruy` バックエンドにより動作保証する。

### 2.3 ツールチェーンおよびリンク方針

- **Rust ツールチェーン**: `rust-toolchain.toml` にてバージョンを固定（現在は 1.99.1）。
- **静的リンク優先**:
  - Windows: `.cargo/config.toml` にて `target.x86_64-pc-windows-msvc` および `target.aarch64-pc-windows-msvc` に `+crt-static` を設定し、CRT を静的リンクする。
  - Linux: 外部の共有ライブラリ（C++ ランタイム等）への不要な動的リンクを最小化する。

### 2.4 パス解決・設定上書き方針

> **現在仕様に関する注記**: 本ADRは判断理由の正本であり、現在仕様の正本ではない。現在の仕様は [ADR-008 環境パスと設定上書きの実装整合性](./008-environment-path-and-overrides.md)、[TT-DS-001 データ構造仕様書](../design/TT-DS-001_データ構造仕様書.md)、[TT-ENV-001 環境・開発基盤仕様書](../setup/TT-ENV-001_環境・開発基盤仕様書.md) を参照する。以下の旧記述は、ADR-002の判断時点における記録として保持する。

- **データ・モデル配置場所**:
  - 各 OS の標準ディレクトリ仕様に準拠する。
    - Windows: `%LOCALAPPDATA%\tray-translate`
    - Linux: `~/.local/share/tray-translate` (XDG Base Directory 準拠)
    - macOS: `~/Library/Application Support/tray-translate`
- **環境変数による上書き**:
  - すべての主要設定およびパスは環境変数接頭辞 **`TT_*`** で上書き可能とする。
    - `TT_MODEL_DIR`: モデル保存先ディレクトリの上書き
    - `TT_CONFIG_DIR`: 設定保存先ディレクトリの上書き
    - `TT_LOG_LEVEL`: ログレベル（error, warn, info, debug, trace）の上書き
    - `TT_COMPUTE_TYPE`: 推論計算精度（int8, int8_float32, auto 等）の上書き

### 2.5 モデル管理およびネットワークポリシー

- **リポジトリ分離**: 翻訳モデルファイル（.argosmodel や重み）は Git リポジトリに含めない（`.gitignore` で除外）。
- **完全オフライン動作**:
  - **翻訳推論時の外部ネットワーク通信は一切行わない（完全オフライン）**。
  - ネットワーク通信が発生するのは、ユーザーが明示的にモデルの新規ダウンロードや更新を指示した時のみとする。

## 3. 影響・トレードオフ

- **メリット**:
  - 外部ランタイムや GPU ドライバに依存しないため、一般ユーザーの PC 環境に導入しやすく、トラブルが少ない。
  - 常駐時のメモリ消費が少なく、起動が極めて高速。
  - Rocky 8 系での Linux ビルドにより、幅広い Linux ディストリビューションで単一バイナリが動作する。
- **トレードオフ**:
  - GPU 推論に比べると大規模な一括翻訳（数十万文字の書籍丸ごと等）では処理時間がかかる（※本ツールの用途は日常的な選択テキストのクリップボード翻訳であり、実測 0.07秒/文 の速度は十分に要求を満たす）。

## 4. 改訂履歴 (Change Log)

| 版数    | 改訂日     | 変更者              | 変更内容・変更理由 (Why)                                                      |
| :------ | :--------- | :------------------ | :---------------------------------------------------------------------------- |
| Rev.1.0 | 2026-10-20 | tray-translate team | 新規作成（対象OS、CPU、リンク方針、パスおよびモデル通信方針の初版制定）       |
| Rev.1.1 | 2026-10-27 | tray-translate team | ADR-008および現在仕様の参照注記を追加し、macOS GUIの技術名をegui/eframeへ訂正 |