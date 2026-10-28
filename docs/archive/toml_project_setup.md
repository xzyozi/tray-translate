---
title: "プロジェクトセットアップガイド（Hatch版）"
document_type: "operation_procedure"
document_role: "procedure"
ssot_scope: "environment"
status: "deprecated"
version: "1.0"
created_at: "2026-10-20"
updated_at: "2026-10-27"
author: "xzyozi"
owner: "xzyozi"
purpose: "Python の環境を Hatch で構築する手順を保管する。このリポジトリには適用されないため廃止した"
related_documents:
  - "ドキュメントガバナンス（相対パス: ../README.md）"
---

> **廃止（deprecated）**: この文書は、別のプロジェクトのテンプレートから持ち込まれた記述で、このリポジトリには適用されない。
>
> - **理由**: Hatch の `hatch run setup`、`requirements.in`、Playwright のブラウザドライバを前提としているが、これらはこのリポジトリに存在しない。ルートの `pyproject.toml` は、Argos との出力比較（`bench/`）専用の Python 設定で、Hatch は使っていない。
> - **代替**: ビルドと検証の環境は、環境・開発基盤仕様書（#102）で定義する。Python 検証環境は `docs/test/argos-baseline-result.md` の計測条件を参照する。

# プロジェクトセットアップガイド (Hatch版)

このプロジェクトは、依存関係と開発環境の管理に [Hatch](https://hatch.pypa.io/latest/) を使用します。プロジェクトルートにある `pyproject.toml` ファイルが、従来の `setup.py` の役割を置き換えます。

## 初回のみ必要な準備

作業を始める前に、Hatchをインストールする必要があります。この作業は一度だけで結構です。

```shell
pip install hatch
```

## プロジェクトのセットアップ

プロジェクトの環境構築、すべての依存関係のインストール、Playwrightが必要とするブラウザのダウンロードを行うには、プロジェクトのルートディレクトリで以下のコマンドを実行してください。

```shell
hatch run setup
```

このコマンド一つで、以下の処理が自動的に実行されます。

1.  プロジェクト専用の仮想環境がなければ作成します。
2.  `pyproject.toml` に指定された、アプリケーション用および開発用のすべての依存関係をインストールします。
3.  `[tool.hatch.scripts]` に定義されたセットアップスクリプトを実行します。これには以下の処理が含まれます。
    *   `requirements.in` から `requirements.txt` を生成する。
    *   `requirements.txt` の内容と環境を完全に同期させる。
    *   Playwrightが必要とするブラウザドライバをインストールする。

## 仮想環境のアクティベート

プロジェクトの環境内で作業（例：スクリプトの手動実行など）を行いたい場合は、以下のコマンドで仮想環境のシェルに入ることができます。

```shell
hatch shell
```

## テストの実行

このプロジェクトでは、`pytest` を使用してテストを実行するよう設定されています。以下のコマンドでテストスイート全体を実行できます。

```shell
hatch run test
```

---
