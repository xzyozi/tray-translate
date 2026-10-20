#!/usr/bin/env bash
# プロジェクトCI契約: .github/workflows/ci.yml から呼び出される。
# Rustのバージョンは rust-toolchain.toml で固定され、rustup が自動で取得する。
set -euo pipefail

if [ ! -f "Cargo.toml" ]; then
  echo "Cargo.toml not found; Rust checks are skipped."
  exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found. Install rustup (https://rustup.rs) and retry." >&2
  exit 1
fi

cargo --version
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
