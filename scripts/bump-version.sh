#!/usr/bin/env bash
# Single-source-of-truth version bump script for Z Engine workspace.
# Usage: ./scripts/bump-version.sh <version>
# Example: ./scripts/bump-version.sh 1.4.1

set -euo pipefail
cd "$(dirname "$0")/.."

if [ $# -ne 1 ]; then
  echo "Usage: $0 <new-version> (e.g. 1.4.1)"
  exit 1
fi

NEW_VER="$1"
# Strip leading 'v' if provided
NEW_VER="${NEW_VER#v}"

# Validate semver format
if ! [[ "$NEW_VER" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "Error: '$NEW_VER' is not a valid semver version (e.g. 1.4.1)"
  exit 1
fi

echo "Bumping Z Engine workspace version to v$NEW_VER..."

# 1. Update Cargo.toml workspace version
perl -i -pe 's/^(version\s*=\s*)".*"/$1"'"$NEW_VER"'"/' Cargo.toml

# 2. Update tauri.conf.json version
perl -i -pe 's/("version"\s*:\s*)".*"/$1"'"$NEW_VER"'"/' crates/z-engine-gui/src-tauri/tauri.conf.json

# 3. Update Cargo.lock
cargo check --workspace --quiet

echo "Successfully updated version to v$NEW_VER in:"
echo " - Cargo.toml"
echo " - crates/z-engine-gui/src-tauri/tauri.conf.json"
echo " - Cargo.lock"
