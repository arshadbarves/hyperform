#!/usr/bin/env bash
# Package the desktop app into a macOS .app bundle (ad-hoc signed).
# Prereqs: release build of zengine-gui + ui/dist present.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN="target/release/zengine-gui"
[ -x "$BIN" ] || { echo "missing $BIN — run: cargo build --release --workspace"; exit 1; }
[ -d "crates/z-engine-gui/ui/dist" ] || { echo "missing ui/dist — run: (cd crates/z-engine-gui/ui && npm run build)"; exit 1; }

APP="target/release/bundle/macos/HyperForm AI.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/zengine-gui"
cp crates/z-engine-gui/src-tauri/icons/icon.icns "$APP/Contents/Resources/icon.icns"
cp crates/z-engine-gui/src-tauri/icons/icon.png "$APP/Contents/Resources/icon.png"

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>              <string>HyperForm AI</string>
    <key>CFBundleDisplayName</key>       <string>HyperForm AI</string>
    <key>CFBundleIdentifier</key>        <string>ai.hyperform.gui</string>
    <key>CFBundleExecutable</key>        <string>zengine-gui</string>
    <key>CFBundleIconFile</key>          <string>icon</string>
    <key>CFBundlePackageType</key>       <string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleVersion</key>           <string>1</string>
    <key>LSMinimumSystemVersion</key>    <string>10.15</string>
    <key>NSHighResolutionCapable</key>   <true/>
    <key>NSAppTransportSecurity</key>
    <dict>
        <key>NSAllowsArbitraryLoads</key><true/>
    </dict>
</dict>
</plist>
PLIST

codesign --force --deep -s - "$APP" 2>/dev/null || true
echo "packaged: $APP"
