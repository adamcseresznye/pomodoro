#!/usr/bin/env bash
# Package the x86_64 release build as a .tar.gz and a .deb. Run on Linux.
set -euo pipefail
version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
binary="target/x86_64-unknown-linux-gnu/release/pomodoro"
mkdir -p dist

stage="dist/pomodoro-soundscapes-linux-x64"
rm -rf "$stage"
mkdir -p "$stage"
install -m755 "$binary" "$stage/pomodoro-soundscapes"
install -m644 packaging/linux/pomodoro-soundscapes.desktop "$stage/"
install -m644 assets/app-icon.png "$stage/pomodoro-soundscapes.png"
install -m644 README.md "$stage/"
tar -czf dist/pomodoro-soundscapes-linux-x64.tar.gz -C dist pomodoro-soundscapes-linux-x64
rm -rf "$stage"

root="$(mktemp -d)"
install -Dm755 "$binary" "$root/usr/bin/pomodoro-soundscapes"
install -Dm644 packaging/linux/pomodoro-soundscapes.desktop "$root/usr/share/applications/pomodoro-soundscapes.desktop"
install -Dm644 assets/app-icon.png "$root/usr/share/icons/hicolor/256x256/apps/pomodoro-soundscapes.png"
mkdir -p "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<CONTROL
Package: pomodoro-soundscapes
Version: $version
Section: utils
Priority: optional
Architecture: amd64
Depends: libasound2 | libasound2t64, libgl1, libxkbcommon0
Maintainer: Adam Cseresznye <csenye.chem@gmail.com>
Description: Pomodoro timer with offline ambient soundscapes
 A desktop focus timer with rain, ocean, forest and noise layers.
CONTROL
dpkg-deb --build --root-owner-group "$root" "dist/pomodoro-soundscapes_${version}_amd64.deb"
