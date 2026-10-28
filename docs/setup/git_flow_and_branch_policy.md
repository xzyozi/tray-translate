---
title: "開発ワークフロー ＆ ブランチ運用規約"
document_type: "operation_procedure"
document_role: "procedure"
ssot_scope: "environment"
status: "review"
version: "1.0"
created_at: "2026-10-20"
updated_at: "2026-10-27"
author: "xzyozi"
owner: "xzyozi"
purpose: "GitHub Flow の開発サイクル、ブランチ命名、コミットメッセージ、ブランチの自動整理の規約を定義するため"
related_documents:
  - "ドキュメントガバナンス（相対パス: ../README.md）"
---

# 開発ワークフロー ＆ ブランチ運用規約

本ドキュメントは、プロジェクトにおける **GitHub Flow 開発サイクル**、**ブランチ命名規則**、および **GitHub / CI/CD / ローカル Git** を活用したブランチの自動クリーンアップ・管理仕様を定義します。

---

## 1. 開発サイクル (GitHub Flow 概要)

```
Issue → ブランチ → 実装・検証 → PR → CI確認 → マージ(main)
```

### 主要ブランチ (Long-lived Branch)
- **`main`**: メインブランチ。CI が常時パスし、安定してビルド可能な状態を保持します。直接 push は禁止し、必ず PR を経由してマージします。

### 作業ブランチ (Short-lived Branches)
- `main` から分岐し、作業完了後に `main` へ向けて PR を作成します。
- **命名規則**: 英数字とハイフンのみを使用します。スラッシュ（`/`）は使わず、種別を先頭に付けたハイフン区切りとします。
  - **`feat-<機能名>`**: 新機能追加用ブランチ
  - **`fix-<修正内容>`**: 不具合修正用ブランチ
  - **`docs-<文書名>`**: ドキュメント・ADR 追加・更新専用ブランチ
  - **`other-<作業内容>`**: CI・インフラ・リポジトリ規約・初期設定用ブランチ

### コミットメッセージ規約
- 書式: `[種別] 概要`
  - 種別: `feat` / `fix` / `docs` / `other`
  - 概要: 日本語で簡潔に記載（例: `[docs] 対象プラットフォームのADR-002を追加`）

### コード・ファイル規約
- 改行コード: **LF**（`.gitattributes` で固定）
- 文字コード: **UTF-8 (BOMなし)**
- コミット前に `git diff --check` で不要な空白・改行コード不正がないか確認します。

---

## 2. CI/CD ＆ リモート自動化 (GitHub / GitHub Actions)

不要なリモートブランチが残留しないよう、GitHub 標準機能で自動クリーンアップします。

### GitHub 設定: PRマージ後のリモートブランチ自動削除
- GitHubの管理画面 (`Settings` > `General` > `Pull Requests`) にて **`Automatically delete head branches`** を有効化。
- PRが `main` にマージされた時点で、リモートの作業ブランチが自動削除されます。

---

## 3. ローカル Git 開発環境の自動化設定

リモートで削除されたブランチがローカル環境に古い追跡情報 (`[gone]`) として残るのを防ぐため、以下の設定を推奨します。

### ① `git fetch` への自動 Prune 設定
以下のコマンドを実行すると、`git fetch` や `git pull` を行うたびに、リモートで消えたブランチのローカル追跡情報を自動的に削除します。

```bash
git config --global fetch.prune true
```

### ② ローカル `[gone]` ブランチを一括削除する Git エイリアス
追跡元リモートブランチが消えたローカルブランチを一括で整理・削除するコマンドを追加します。

```bash
# エイリアスの設定 (PowerShell / Git Bash 共通)
git config --global alias.cleanup "!git branch -vv | grep '\[gone\]' | awk '{print $1}' | xargs -r git branch -d"
```

#### 使い方
```bash
git cleanup
```
実行すると、リモートで既にマージ・削除されたローカルブランチが一括で安全に削除されます。
