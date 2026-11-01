#!/usr/bin/env bash
# Build dist/Tele-Task-Monitor.app (menu-bar only app, no Dock icon).
#
#   scripts/bundle-macos.sh              # for this Mac's architecture
#   scripts/bundle-macos.sh --universal  # Apple Silicon + Intel in one binary
#
# Requires: Rust (rustup), Xcode Command Line Tools (xcode-select --install).
set -euo pipefail
cd "$(dirname "$0")/.."

APP_NAME="Tele-Task-Monitor"
BIN="tele-task-monitor"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
APP="dist/${APP_NAME}.app"

if [[ "${1:-}" == "--universal" ]]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  mkdir -p target/universal
  lipo -create -output "target/universal/${BIN}" \
    "target/aarch64-apple-darwin/release/${BIN}" "target/x86_64-apple-darwin/release/${BIN}"
  BUILT="target/universal/${BIN}"
else
  cargo build --release
  BUILT="target/release/${BIN}"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BUILT" "$APP/Contents/MacOS/${BIN}"
cp -R web "$APP/Contents/Resources/web"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key><string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key><string>com.kataruna.tele-task-monitor</string>
  <key>CFBundleExecutable</key><string>${BIN}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Ad-hoc signature so Apple Silicon runs it and the firewall remembers "Allow".
# (Sharing with other people needs a Developer ID signature + notarization instead.)
codesign --force --deep --sign - "$APP"

echo "Built $APP"
echo "Run:    open \"$APP\""
echo "Install: cp -R \"$APP\" /Applications/"
