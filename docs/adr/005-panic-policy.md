---
title: "panic 方針（リリースプロファイルの panic 設定）"
document_type: "architecture_decision_record"
document_number: "ADR-005"
document_role: "rationale"
ssot_scope: "decision_rationale"
status: "proposed"
version: "1.0"
created_at: "2026-10-23"
updated_at: "2026-10-23"
author: "tray-translate team"
owner: "tray-translate team"
purpose: "常駐アプリの panic 時の挙動と、言語コードの前提を決定するため"
related_documents:
  - "ドキュメントガバナンス（相対パス: ../README.md）"
  - "用語集（相対パス: ../../CONTEXT.md）"
  - "対象プラットフォーム方針（相対パス: ./002-target-platform-and-environment-policy.md）"
---

# ADR-005: panic 方針（リリースプロファイルの panic 設定）

| 項目           | 内容                |
| :------------- | :------------------ |
| ステータス     | proposed            |
| 決定日         | 未定（承認待ち）    |
| 決定者・承認者 | tray-translate team |
| 置換元ADR      | なし                |
| 関連Issue      | #51                 |

## 1. 背景・課題

- ルート `Cargo.toml` のリリースプロファイルは `panic = "abort"` である。
- 常駐 GUI の翻訳ワーカースレッド内（CTranslate2 / SentencePiece の周辺を含む）で panic が起きると、プロセス全体が即終了する。
- `panic` はワークスペース共通のプロファイル設定であり、パッケージ単位（GUI のみ unwind）では指定できない。
- 判断しない場合: 方針が暗黙のままになり、`unwrap` などの扱いが場当たり的になる。

## 2. 決定（案）

1. リリースプロファイルは `panic = "abort"` を維持する。
2. core / app では、外部入力・FFI・ファイル I/O に由来するエラーを `Result` で返す。`unwrap` / `expect` は、直前の処理で成立が保証される不変条件の箇所に限る。
3. GUI のワーカーは、エラーを `Event::Error` として UI へ通知する。
4. 言語コードの前提: 言語コードに `_` を含めない。ペアキーは `<from>_<to>` とする（`split('_')` で一意に分解できる必要がある）。Argos Translate の言語コード（`en`, `ja`, `zh` など）はこの前提を満たす。

## 3. 理由・代替案

### 採用理由

1. バイナリサイズと速度の利点を維持できる。
2. panic の発生源を `Result` 化で減らす方が、`unwind` で隠すより原因を追いやすい。

### 代替案

| 代替案 | 利点 | 欠点・リスク | 採用しなかった理由 |
| :-- | :-- | :-- | :-- |
| `panic = "unwind"` に変更 | ワーカーの panic を捕捉して UI を維持できる | FFI 境界を越える unwind は未定義動作になりうる。サイズが増える | FFI（CT2）周辺の安全性を確認できていない |
| GUI のみ unwind | 影響範囲を限定できる | プロファイルがワークスペース共通のため実現しにくい | 設定上の制約 |

## 4. 影響・トレードオフ

- **利点**: 現状の設定を変えずに方針を明文化できる。
- **欠点・コスト**: ワーカーで panic が起きると、アプリは終了する。
- **移行・ロールバック**: `Cargo.toml` の設定変更のみで切り替えられる。

## 5. 関連文書・後続作業

- **要求・Issue**: #51
- **後続作業**: ワーカー内の panic 経路（`unwrap` など）の棚卸し

## 6. 例外・見直し条件

- **見直し条件**: ワーカー起因の異常終了が実際に報告されたとき。FFI 境界の unwind の安全性を確認できたとき。

## 7. 改訂履歴 (Change Log)

| 版数    | 改訂日     | 変更者            | 変更内容・変更理由 (Why) |
| :------ | :--------- | :---------------- | :----------------------- |
| Rev.1.0 | 2026-10-23 | tray-translate team | 新規作成（案）         |
