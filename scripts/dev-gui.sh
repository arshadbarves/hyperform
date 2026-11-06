#!/usr/bin/env bash
# Launch HyperForm AI with Vite HMR. Save a UI file and the window updates.
# First run compiles Rust (slow); later UI edits hot-reload.
set -euo pipefail
cd "$(dirname "$0")/.."

UI=crates/z-engine-gui/ui
if [[ ! -d "$UI/node_modules" ]]; then
  (cd "$UI" && npm install)
fi

# CLI must start from a folder that *contains* src-tauri/ (not from ui/).
cd crates/z-engine-gui
exec ./ui/node_modules/.bin/tauri dev "$@"
