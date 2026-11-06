#!/usr/bin/env bash
# Pre-tag gate: fmt + clippy -D warnings + full test suite.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== cargo fmt --check =="
cargo fmt --all -- --check

echo "== cargo clippy --workspace --all-targets -- -D warnings =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== cargo test --workspace =="
cargo test --workspace

echo "ALL CHECKS PASSED"
