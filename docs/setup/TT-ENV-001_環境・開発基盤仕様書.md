---
title: "環境・開発基盤仕様書"
document_type: "environment_specification"
document_number: "TT-ENV-001"
source_issue: "#102"
document_role: "normative"
ssot_scope: "environment"
status: "draft"
version: "1.0"
created_at: "2026-10-27"
updated_at: "2026-10-27"
author: "tray-translate team"
owner: "tray-translate team"
purpose: "tray-translate の開発・検証・リリース環境、品質ゲート、依存関係の再現条件を定義するため"
related_documents:
  - "ドキュメントガバナンス（相対パス: ../README.md）"
  - "基本設計書（相対パス: ../design/TT-BD-001_基本設計書.md）"
  - "対象プラットフォームおよび環境非依存方針（相対パス: ../adr/002-target-platform-and-environment-policy.md）"
  - "G3 の合格基準（相対パス: ../adr/003-g3-acceptance-criteria.md）"
  - "GUI のプラットフォーム依存処理の境界とウィンドウのライフサイクル（相対パス: ../adr/007-gui-platform-boundary-and-window-lifecycle.md）"
  - "開発ワークフロー・ブランチ運用規約（相対パス: ./git_flow_and_branch_policy.md）"
  - "Mermaid CI 検証運用（相対パス: ./mermaid_ci_validation.md）"
  - "実モデルを使った確認手順（相対パス: ../test/real-model-verification.md）"
  - "Argos Translate 基準値計測結果（相対パス: ../test/argos-baseline-result.md）"
  - "用語集（相対パス: ../../CONTEXT.md）"
---

# 環境・開発基盤仕様書
**tray-translate の開発・検証・リリース環境**

---

## 1. 目的・対象範囲

- **目的**: 本書は、tray-translate の開発、GitHub Actions による検証、リリース、Python による比較検証に使用する環境と再現条件を定義する。環境・開発基盤に関する正本として、プラットフォーム、言語・ランタイム、ツールチェーン、依存関係、ビルド、CI/CD、設定、再現性を扱う。
- **対象環境**: Windows/Linux のローカル開発環境、プルリクエスト（PR）CI、`main` への push 時の CI、手動実行 CI、タグまたは手動実行によるリリース、docs の Mermaid 検証、Argos との出力比較を行う Python benchmark 環境。
- **対象範囲**: Rust/Cargo workspace、Rust GUI の feature 構成、CTranslate2/SentencePiece を含む翻訳実行基盤、GitHub Actions の実行条件と品質ゲート、リリース成果物、`bench/` の検証専用 Python 環境を対象とする。現行 workflow の実行条件は、`.github/workflows/ci.yml`、`.github/workflows/release.yml`、`.github/workflows/validate-mermaid.yml` の設定を正本とする。
- **対象外**: アプリケーションの機能仕様、モジュール間の責務、データ構造、翻訳モデルの内容・ライセンス、Git のブランチ操作手順、Mermaid 図の内容の妥当性、脆弱性検査や依存自動更新の導入方針は本書で定義しない。これらは関連文書を参照するか、別途判断する。

## 2. ハード要件・制約

- **必須プラットフォーム・ハードウェア**:
  - 現行の実行・配布対象は Windows 10/11 x86_64 と、RHEL 8 系または Rocky Linux 8 系互換の Linux x86_64 である。Windows は MSVC ツールチェーンを使用する。
  - 翻訳推論は CPU のみで実行し、GPU、CUDA、ROCm は使用しない。CPU の命令セット差を含む対象プラットフォーム方針の理由は [ADR-002](../adr/002-target-platform-and-environment-policy.md) を参照する。
  - macOS arm64 と Linux arm64 は ADR-002 の Tier 2 であり、本書の現行 CI・リリース成果物の対象ではない。
- **互換性・契約・規制**:
  - Linux の配布用ビルドは `ubuntu-latest` runner 上の `rockylinux:8` container で行い、glibc 2.28 を基準とする。新しい glibc の環境でビルドした成果物を Linux 配布物の基準にはしない。
  - Windows MSVC の x86_64 と aarch64 target には `.cargo/config.toml` の `+crt-static` を適用する。現行 release workflow の Windows 成果物は x86_64 である。
  - GUI は Windows の配布物に含める。Linux では GUI の通常 feature と `--no-default-features` を CI で検査するが、配布物は CLI のみとする。
- **ネットワーク・アクセス制約**:
  - 翻訳推論は完全オフラインで実行する。モデルの一覧取得・ダウンロードをユーザーが明示的に行う場合だけ、アプリケーションの通信が発生する。
  - CI/CD はソース、依存関係、GitHub Actions、リリース成果物の取得・公開にネットワークを使用する。Python benchmark は `--install` を指定した場合に Argos のモデル情報・パッケージ取得でネットワークを使用する。
- **その他の制約**:
  - 翻訳モデルファイルは Git リポジトリおよびリリース成果物へ含めない。モデルの保存場所と環境変数による上書きは 4.5 に記載する。
  - リリース成果物のファイル形式と対象バイナリは 4.3 に定める。Linux GUI を配布しないことは、Linux GUI をビルドできないことを意味しない。

## 3. 環境概要

| 環境                     | 用途                                                 | プラットフォーム / 実行方式                                                         | 所有者              | 更新・廃止方針                                                                        |
| :----------------------- | :--------------------------------------------------- | :---------------------------------------------------------------------------------- | :------------------ | :------------------------------------------------------------------------------------ |
| ローカル開発             | Rust の実装、単体テスト、手動検証                    | Windows 10/11 x86_64 または Linux x86_64、rustup/Cargo                              | tray-translate team | `rust-toolchain.toml`、`Cargo.toml`、`Cargo.lock` の変更に合わせて更新する            |
| PR CI                    | `main` 向け PR の Rust 検証                          | GitHub Actions、`ubuntu-latest` 1 runner                                            | tray-translate team | `.github/workflows/ci.yml` の path filter または matrix を変更したときに更新する      |
| `main` push CI / 手動 CI | `main` push 後または手動実行時の Rust 検証           | GitHub Actions、`ubuntu-latest` と `windows-latest`                                 | tray-translate team | `.github/workflows/ci.yml` の event、matrix、チェックを変更したときに更新する         |
| Release                  | タグまたは手動実行による配布物のビルドと公開         | Linux: `ubuntu-latest` runner + `rockylinux:8` container、Windows: `windows-latest` | tray-translate team | `.github/workflows/release.yml` の対象 OS、成果物、公開方法を変更したときに更新する   |
| Mermaid 検証             | docs Markdown に含まれる Mermaid の構文・render 検証 | GitHub Actions、`ubuntu-latest`、Node.js 22.14.0、Mermaid CLI 11.4.2                | tray-translate team | `.github/workflows/validate-mermaid.yml` または検証スクリプトを変更したときに更新する |
| Python benchmark         | Argos Translate との出力・性能比較                   | ローカルの Python 3.10 以上、CPU 専用、`bench/argos_baseline.py`                    | tray-translate team | `pyproject.toml` または benchmark スクリプトの依存・実行条件を変更したときに更新する  |

## 4. 適用する環境要素

### 4.1 言語・ランタイム・実行基盤

- **Rust**: `rust-toolchain.toml` で channel `1.99.1` を固定する。`rustfmt` と `clippy` を components に含め、profile は `minimal` とする。Cargo package の edition は workspace で `2021` とする。
- **Cargo workspace**: workspace member は `crates/app`、`crates/core`、`crates/cli`、`crates/gui` である。`default-members` は `crates/app`、`crates/core`、`crates/cli` であり、GUI は member だが default-members ではない。GUI の build、clippy、test は `-p tray-translate-gui` で明示する。`spike/ct2-build` は workspace から exclude する。
- **C/C++ build runtime**: CTranslate2 系の C/C++ ビルドが必要な環境では、Windows MSVC の CRT 静的リンクを `.cargo/config.toml` で適用する。Linux release container では gcc/g++ と gcc-toolset-12 を導入する。
- **Python**: Python は本体の実行環境ではない。ルート `pyproject.toml` の `requires-python = ">=3.10"` と optional dependency `bench` を、`bench/argos_baseline.py` による Argos Translate との比較検証だけに使用する。`tool.uv.package = false` のため、この Python プロジェクト自体をアプリケーションとして build/install しない。
- **Node.js**: Node.js は Mermaid 検証 workflow 内だけで使用し、version `22.14.0` を指定する。Rust 本体、リリースバイナリ、Python benchmark の実行環境ではない。

### 4.2 ツールチェーン・依存関係

- **Rust 依存管理**: 依存関係は root `Cargo.toml` の `[workspace.dependencies]` と各 crate の package 定義で管理し、解決結果を `Cargo.lock` で管理する。CI と release の Cargo コマンドには `--locked` を付け、lock file と解決結果の変更を無断で許容しない。
- **翻訳基盤**: `ct2rs` は version `=0.10.1` に厳密固定し、`default-features = false`、`sentencepiece` と `ruy` feature を有効にする。CTranslate2 と SentencePiece は本プロジェクトが個別に別管理するのではなく、ct2rs 経由で利用する。
- **Windows link 方針**: `.cargo/config.toml` の `target.x86_64-pc-windows-msvc` と `target.aarch64-pc-windows-msvc` に `-C target-feature=+crt-static` を設定する。これは CTranslate2/SentencePiece の C++ 静的ライブラリと Rust 側 CRT の不一致を避けるためである。
- **GUI feature**: `crates/gui/Cargo.toml` の default feature は `tray` と `hotkey` である。通常構成では両方を有効にし、Ubuntu の追加検査では `--no-default-features` で両方を無効にする。Linux の `tray` は `ksni` backend を使用する依存定義になっている。
- **CI のキャッシュ**: Ubuntu の Rust CI では `mozilla-actions/sccache-action@v0.0.9` と Rust cache を使用し、Rust/CMake のコンパイラ launcher を sccache に設定する。キャッシュは再実行を高速化するためのもので、依存関係の正本ではない。
- **Python 検証依存**: `pyproject.toml` の `bench` は `argostranslate==1.11.0` と CPU 用 PyTorch index の `torch` を定義する。`numpy<2.4` の制約は検証機の CPU 互換性のためである。`dev` の pytest、ruff、mypy は Python 検証用の開発依存であり、現行 GitHub Actions の品質ゲートではない。
- **旧 Python 環境文書**: Hatch/pip-tools を前提とする旧文書は `docs/archive/` へ移動済みであり、現行の Rust/Cargo および Python benchmark 環境には適用しない。
- **Mermaid 検証依存**: Mermaid workflow は Node.js 22.14.0 を使用し、検証スクリプトは `npx --yes --package @mermaid-js/mermaid-cli@11.4.2 mmdc` で直接 version を指定する。ローカルの `package.json` や `package-lock.json` は前提にしない。

### 4.3 ビルド・パッケージング・配布

- **通常の release profile**: root `Cargo.toml` の release profile は `opt-level = 3`、`lto = "thin"`、`codegen-units = 1`、`strip = true`、`panic = "abort"` とする。
- **Linux release build**: `release.yml` は `rockylinux:8` container 内で `cargo build --release --locked --bin tt` を実行する。成果物 `tray-translate-linux-x86_64.tar.gz` には `tt`、`README.md`、`LICENSE` を含める。
- **Windows release build**: `release.yml` は `windows-latest` で `cargo build --release --locked -p tray-translate-cli -p tray-translate-gui` を実行する。成果物 `tray-translate-windows-x86_64.zip` には `tt.exe`、`tt-gui.exe`、`README.md`、`LICENSE` を含める。
- **リリース契機**: `v*` tag の push または `workflow_dispatch` で release workflow を実行する。tag push の場合は、`v` を除いた tag version と root `Cargo.toml` の workspace version が一致しなければ失敗する。
- **成果物の公開・証跡**: tag 実行では GitHub Release asset として配布物および `.sha256` を upload する。手動実行では GitHub Actions artifact として配布物および `.sha256` を upload する。SHA-256 は配布物の整合性確認用であり、署名ではない。
- **GUI の配布範囲**: Windows は CLI と GUI を配布し、Linux は CLI のみを配布する。Linux GUI の CI 検査は移植性とビルド可能性を確認するために行い、Linux GUI の配布を意味しない。

### 4.4 CI/CD・品質ゲート

Rust CI と Mermaid 検証は別の GitHub Actions workflow として実行する。Rust CI の job は `fail-fast: false` の matrix で動作し、PR では Ubuntu のみ、`main` push と `workflow_dispatch` では Ubuntu と Windows を対象とする。

#### 品質ゲート一覧

| ID     | 品質ゲート                                 | 実行コマンド                                                                                                                                                         | 対象・実行条件                                                                                                                                 | 合格条件                                                             |
| :----- | :----------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------- |
| QG-001 | Rust format check                          | `cargo fmt --all -- --check`                                                                                                                                         | `.github/ci.sh` 経由。PR は Ubuntu、`main` push と手動実行は Ubuntu + Windows                                                                  | コマンドが終了コード 0 で、CI job が成功すること                     |
| QG-002 | Rust clippy                                | `cargo clippy --all-targets --locked -- -D warnings`                                                                                                                 | `.github/ci.sh` 経由。root の default-members、PR は Ubuntu、`main` push と手動実行は Ubuntu + Windows                                         | warning をエラーとして扱い、終了コード 0 であること                  |
| QG-003 | Rust test                                  | `cargo test --locked`                                                                                                                                                | `.github/ci.sh` 経由。root の default-members、PR は Ubuntu、`main` push と手動実行は Ubuntu + Windows                                         | テストが終了コード 0 であること                                      |
| QG-004 | GUI 通常 feature の clippy/test            | Windows: `cargo clippy -p tray-translate-gui --all-targets --locked -- -D warnings`、`cargo test -p tray-translate-gui --locked`。Ubuntu: 同じ2コマンド              | Windows は `main` push と手動実行のみ。Ubuntu は PR、`main` push、手動実行。default feature の `tray` + `hotkey` を対象                        | 対象 OS の両コマンドが終了コード 0 であること                        |
| QG-005 | GUI `--no-default-features` の clippy/test | `cargo clippy -p tray-translate-gui --all-targets --locked --no-default-features -- -D warnings`、`cargo test -p tray-translate-gui --locked --no-default-features`  | Ubuntu のみ。PR、`main` push、手動実行。`tray` と `hotkey` を無効にした構成を対象                                                              | 両コマンドが終了コード 0 であること                                  |
| QG-006 | docs Mermaid 検証                          | 差分: `node .github/scripts/validateMermaidMarkdown.mjs --base "$base_sha" --head "$GITHUB_SHA"`、手動全件: `node .github/scripts/validateMermaidMarkdown.mjs --all` | `docs/**/*.md` が変更された `main` 向け PR/`main` push は差分検証、`workflow_dispatch` は全件検証。Ubuntu、Node.js 22.14.0、Mermaid CLI 11.4.2 | 対象 Mermaid block を正常に SVG render でき、workflow が成功すること |

#### CI 実行条件

- **PR**: `.github/workflows/ci.yml` は `main` 向け PR のうち、`crates/**`、`Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`、`.cargo/**`、`.github/ci.sh`、`.github/workflows/ci.yml` に変更がある場合だけ Rust CI を実行する。matrix は `ubuntu-latest` のみであるため、PR では Windows 検査を行わない。
- **`main` push**: 同じ path filter に該当する `main` への push で Rust CI を実行し、matrix は `ubuntu-latest` と `windows-latest` である。
- **手動実行**: `workflow_dispatch` は Rust CI の path filter を適用せず、Ubuntu と Windows の matrix を実行する。Mermaid workflow の手動実行は docs 全件を対象にする。
- **Ubuntu GUI 依存**: QG-004/QG-005 の前に、Ubuntu job で `libxcb-render0-dev`、`libxcb-shape0-dev`、`libxcb-xfixes0-dev`、`libxkbcommon-dev`、`libssl-dev` を apt から導入する。
- **Release workflow**: `release.yml` は QG-001 から QG-006 の品質ゲートを実行しない。release build 自体は `--locked` で行い、build、package、checksum、upload の各 step が成功することを別途確認する。
- **権限**: Rust CI と Mermaid workflow は `contents: read`、release workflow は `contents: write` を指定する。

### 4.5 設定・秘密情報

- **アプリケーション設定**: モデル保存先は `TT_MODEL_DIR`、設定・履歴保存先は `TT_CONFIG_DIR` で上書きできる。未指定時は OS 標準のデータ・設定ディレクトリを使用する。対象プラットフォームの標準パスと追加の設定上書き項目は [ADR-002](../adr/002-target-platform-and-environment-policy.md) を参照する。
- **モデル取得時の通信**: モデルの一覧取得・ダウンロードを明示的に行う場合は、`HTTPS_PROXY` と `HTTP_PROXY` をプロキシ設定に使用できる。翻訳推論時はこれらの通信を行わない。
- **CI/CD 設定**: GitHub Actions workflow に定義された `contents` permission と event/path filter を適用する。Release の公開に必要な権限は workflow の `contents: write` で管理し、秘密情報の値を本書へ記載しない。
- **秘密情報の扱い**: API token、パスワード、秘密鍵、個人の認証情報、GitHub の認証情報は本書、リポジトリ、ログへ記載しない。必要な秘密情報がある場合の供給元・ローテーション・利用手順は本書の対象外であり、GitHub または利用環境の管理機能で扱う。
- **モデルファイル**: 翻訳モデルはリポジトリへ含めず、利用環境のモデル保存先へ導入する。モデル配布物の checksum/署名検証は未実装である。

### 4.6 再現性・サプライチェーン

- **Rust の固定**: `rust-toolchain.toml` の channel `1.99.1`、`rustfmt`、`clippy`、`minimal` profile を使用する。
- **依存関係の固定**: `Cargo.lock` を管理し、CI と release の Cargo コマンドに `--locked` を付ける。`ct2rs` は `=0.10.1` に固定する。これらを変更するときは、変更後の lock file と検証結果を同じ変更単位で更新する。
- **ネイティブ build の固定**: Windows の CRT 静的リンク設定を `.cargo/config.toml` に置き、Linux release は `rockylinux:8` container と gcc-toolset-12 を使用する。
- **Action と Mermaid CLI**: GitHub Actions の action は workflow で `@v4`、`@v2`、`@v0.0.9` などの version/tag を明示する。Mermaid CLI は `@mermaid-js/mermaid-cli@11.4.2` を明示する。Actions の commit SHA 固定や Mermaid CLI の npm 依存グラフ全体の lock file 管理は現行設定にない。
- **成果物の完全性**: release workflow は Linux/Windows の配布物ごとに SHA-256 checksum file を生成して添付する。これは配布物の完全性確認であり、署名、provenance、依存関係の監査を代替しない。
- **未導入の検査・自動更新**: `cargo audit`、`cargo deny`、Dependabot、Renovate は現行の workflow・設定に導入されていない。脆弱性検査、ライセンス判定、SBOM 生成、依存の自動更新について、今回の仕様書では導入方針を決定しない。
- **Python 検証の位置づけ**: Python benchmark は本体の build/runtime ではなく補助検証であり、現行 GitHub Actions の品質ゲートではない。benchmark の実行結果は [Argos Translate 基準値計測結果](../test/argos-baseline-result.md) などの検証証跡で管理する。

## 5. 検証方法

環境仕様の適合は、workflow の成功結果とリポジトリ内の固定設定を組み合わせて確認する。Rust の品質ゲートは 4.4 の QG 表と同じコマンドを使い、workflow の step ログを証跡とする。

| 確認項目                       | 確認方法                                                                                                                                                     | 合格条件                                                                                                                               | 証跡                                                                                     |
| :----------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------- |
| Rust toolchain                 | `rust-toolchain.toml` の channel、components、profileを確認し、CI の Cargo step を実行する                                                                   | channel `1.99.1`、`rustfmt`、`clippy`、`minimal` が設定され、QG-001〜QG-003 が成功する                                                 | `rust-toolchain.toml`、`.github/ci.sh`、CI job log                                       |
| Cargo workspace と依存 lock    | `Cargo.toml` の workspace/default-members/exclude、`Cargo.lock`、`--locked` の workflow command を確認する                                                   | member、default-members、exclude、ct2rs version、lock file の整合が保たれ、locked command が成功する                                   | `Cargo.toml`、`Cargo.lock`、`.github/workflows/ci.yml`、`.github/workflows/release.yml`  |
| QG-001                         | `cargo fmt --all -- --check` を CI で実行する                                                                                                                | 終了コード 0                                                                                                                           | Rust CI の `Run project CI script` step log                                              |
| QG-002                         | `cargo clippy --all-targets --locked -- -D warnings` を CI で実行する                                                                                        | default-members が warning なしで成功                                                                                                  | Rust CI の `Run project CI script` step log                                              |
| QG-003                         | `cargo test --locked` を CI で実行する                                                                                                                       | default-members の test が成功                                                                                                         | Rust CI の `Run project CI script` step log                                              |
| QG-004                         | GUI の通常 feature で Windows/Ubuntu の clippy と test を実行する                                                                                            | 対象 event・OS の両コマンドが成功                                                                                                      | Rust CI の `GUI checks (clippy / test)` step log                                         |
| QG-005                         | Ubuntu で GUI の `--no-default-features` clippy と test を実行する                                                                                           | 対象 event の両コマンドが成功                                                                                                          | Rust CI の `GUI checks without tray / hotkey` step log                                   |
| QG-006                         | `validateMermaidMarkdown.mjs` を PR/push の差分または手動全件モードで実行する                                                                                | 対象 Mermaid block の render と workflow が成功                                                                                        | `validate-mermaid` workflow log                                                          |
| Linux GUI の native dependency | Ubuntu job の apt install step を確認し、その後の GUI checks を実行する                                                                                      | `libxcb-render0-dev`、`libxcb-shape0-dev`、`libxcb-xfixes0-dev`、`libxkbcommon-dev`、`libssl-dev` が導入され、QG-004/QG-005 が成功する | Rust CI の `Install GUI build dependencies (Ubuntu)` step log                            |
| Release 成果物                 | `release.yml` の tag/manual workflow を実行し、生成物と `.sha256` を確認する                                                                                 | Linux は CLI tar.gz、Windows は CLI+GUI zip が生成され、checksum file が添付される                                                     | GitHub Release asset または Actions artifact、`.sha256`                                  |
| Python benchmark               | Python 3.10 以上で `uv run --extra bench python bench/argos_baseline.py --from en --to ja` を実行する。Argos package が未導入の場合だけ `--install` を付ける | CPU 専用で出力 JSON が生成され、比較検証時は [ADR-003](../adr/003-g3-acceptance-criteria.md) の手順・合格基準に従う                    | `bench_en_ja.json` などの benchmark JSON、`docs/test/argos-baseline-result.md`、実行ログ |

QG-006 は Mermaid の構文と SVG render の可否を検証するが、図の業務上・設計上の意味、矢印の妥当性、用語の正しさを判定しない。これらは PR review の責務であり、運用手順は [Mermaid CI 検証運用](./mermaid_ci_validation.md) を参照する。

## 6. 例外・適用差異

| ID     | 対象                       | 標準との差異                                                                                           | 理由・制約                                                                                 | 影響                                                                            | 代替対応                                                                                       | 解除条件                                                         |
| :----- | :------------------------- | :----------------------------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------ | :--------------------------------------------------------------------------------------------- | :--------------------------------------------------------------- |
| EX-001 | PR の Rust CI              | `main` push/手動実行の Windows 検査を PR では省略し、Ubuntu のみ実行する                               | Windows 検査は時間がかかるため、現行 `ci.yml` が PR の matrix を Ubuntu のみに定義している | Windows 固有の回帰は PR 時点で検出されず、main push または手動実行で検出する    | `main` push または `workflow_dispatch` の Ubuntu + Windows matrix で QG-001〜QG-005 を実行する | PR matrix に Windows を追加したとき                              |
| EX-002 | Cargo の default-members   | GUI を default-members から除外し、QG-002/QG-003 の通常 command に含めない                             | GUI は OS 依存の feature と native dependency を持つため、明示的な別検査として管理する     | default-members の clippy/test 成功だけでは GUI の品質を保証しない              | QG-004 と QG-005 で GUI の通常構成・feature 無効構成を検査する                                 | workspace の default-members または GUI の検査方式を変更したとき |
| EX-003 | Linux GUI 配布             | Linux GUI は CI で build/test するが、release package には含めない                                     | 現行の配布範囲は Windows が CLI+GUI、Linux が CLI のみである                               | Linux では GUI の実行・配布を保証せず、ビルドと基本検査の範囲に留める           | Linux release は `tt` と README/LICENSE の CLI package とする                                  | Linux GUI の配布方針と release workflow を更新したとき           |
| EX-004 | 依存脆弱性・ライセンス監査 | `cargo audit`、`cargo deny`、SBOM、Dependabot、Renovate を品質ゲートまたは自動更新として導入していない | 現行 repository に対応する workflow・設定がない                                            | 依存脆弱性やライセンスの自動判定を CI の合格条件にしていない                    | Cargo.lock、`--locked`、ct2rs 固定など現行の再現性対策を適用するが、監査の代替とはしない       | 導入を決定し、workflow・設定・本書を更新したとき                 |
| EX-005 | 翻訳モデル配布物           | モデル配布物の checksum/署名検証を実装していない                                                       | 現行実装はモデルのダウンロード・導入を行うが、配布物の hash/署名検証を行わない             | モデルの真正性・完全性を release artifact の SHA-256 と同じ方法では確認できない | モデルは Git と release package に含めず、明示ダウンロードの対象として管理する                 | モデル検証機能と検証証跡を実装したとき                           |

Git のブランチ運用と Mermaid 検証の実行手順は本書へ複製せず、[開発ワークフロー・ブランチ運用規約](./git_flow_and_branch_policy.md) と [Mermaid CI 検証運用](./mermaid_ci_validation.md) に分離して管理する。

## 7. 関連文書

- **ドキュメントガバナンス**: [docs/README.md](../README.md)。文書役割、SSOT、Frontmatter、配置の正本。
- **基本設計書**: [TT-BD-001 基本設計書](../design/TT-BD-001_基本設計書.md)。システム境界と品質方針を参照する。
- **対象プラットフォーム方針**: [ADR-002](../adr/002-target-platform-and-environment-policy.md)。OS、CPU 専用推論、Rocky Linux/glibc、モデル通信方針の判断理由。
- **G3 合格基準**: [ADR-003](../adr/003-g3-acceptance-criteria.md)。Rust と Argos の出力比較方法と合格基準。
- **GUI プラットフォーム境界**: [ADR-007](../adr/007-gui-platform-boundary-and-window-lifecycle.md)。GUI feature とプラットフォーム依存処理の判断理由。
- **Git 運用手順**: [git_flow_and_branch_policy.md](./git_flow_and_branch_policy.md)。GitHub Flow、ブランチ、コミット、改行の手順。環境仕様の正本ではない。
- **Mermaid 検証手順**: [mermaid_ci_validation.md](./mermaid_ci_validation.md)。Mermaid workflow の運用と検証範囲。環境仕様の正本ではない。
- **実モデル確認手順**: [real-model-verification.md](../test/real-model-verification.md)。Linux 検証機でのモデル導入、翻訳、G3 比較の手順。
- **Argos 基準値証跡**: [argos-baseline-result.md](../test/argos-baseline-result.md)。Python benchmark の計測結果。
- **用語集**: [CONTEXT.md](../../CONTEXT.md)。文書管理とプロジェクト用語の正本。

専用の環境構築手順書は現行リポジトリにない。環境構築の実行手順を追加する場合は、`docs/setup/` の procedure 文書として作成し、本書から参照する。

## 8. 作成完了チェック

- [x] `document_number`、`document_role`、`ssot_scope`、`status`、`owner`、`source_issue` を Frontmatter へ設定した。
- [x] `TT-ENV-001` とファイル名を一致させ、#102 を参照元として記録した。
- [x] 目的、対象範囲、対象外、ハード要件・制約を記載した。
- [x] 開発、PR CI、`main` push/手動 CI、release、Mermaid 検証、Python benchmark の環境を区別した。
- [x] Rust toolchain、Cargo workspace、依存関係、ビルド、配布、設定、再現性を記載した。
- [x] QG-001〜QG-006 を ID 付きで定義し、CI の実行条件と証跡を対応付けた。
- [x] PR の Windows 省略、GUI の default-members 外、Linux GUI の非配布、未導入の監査・自動更新、モデル検証未実装を例外として記録した。
- [x] 手順・規約文書を複製せず、Git と Mermaid の既存文書を参照した。
- [x] 秘密情報、認証情報、利用者固有の値を記載していない。

## 9. 改訂履歴 (Change Log)

| 版数    | 改訂日     | 変更者              | 変更内容・変更理由 (Why)                    |
| :------ | :--------- | :------------------ | :------------------------------------------ |
| Rev.1.0 | 2026-10-27 | tray-translate team | #102 に基づく環境・開発基盤仕様書の初版作成 |
