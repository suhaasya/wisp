#!/usr/bin/env bash
# CI-free smoke: same commands LUM-003 should run, with warnings denied.
set -euo pipefail

export RUSTFLAGS="${RUSTFLAGS:--D warnings}"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace

if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check
else
  echo "cargo-deny not installed; skipping deny check"
fi
