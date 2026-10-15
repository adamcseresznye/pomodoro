#!/usr/bin/env bash
# Wrap a release build in an app bundle and disk image. Run on macOS.
# Usage: packaging/build-macos.sh <rust-target> <arm64|x64>
set -euo pipefail
target="$1"
label="$2"
version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
app="dist/Pomodoro Soundscapes.app"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" dist
cp "target/$target/release/pomodoro" "$app/Contents/MacOS/pomodoro"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"

iconset="$(mktemp -d)/app.iconset"
mkdir -p "$iconset"
for size in 16 32 128; do
  sips -z "$size" "$size" assets/app-icon.png --out "$iconset/icon_${size}x${size}.png" > /dev/null
  sips -z "$((size * 2))" "$((size * 2))" assets/app-icon.png --out "$iconset/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/app-icon.icns"

# Ad-hoc signature: lets the app start on Apple Silicon, but isn't notarized.
codesign --force --deep --sign - "$app"

out="dist/pomodoro-soundscapes-macos-$label.dmg"
rm -f "$out"
hdiutil create -volname "Pomodoro Soundscapes" -srcfolder "$app" -ov -format UDZO "$out"
rm -rf "$app"
