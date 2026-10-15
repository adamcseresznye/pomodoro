#!/usr/bin/env bash
# Package the Windows build as a portable .exe and an .msi installer.
# Run from the repository root in Git Bash, after `cargo build --release`.
# Usage: packaging/build-windows.sh <rust-target>
set -euo pipefail
target="$1"
version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
exe="target/$target/release/pomodoro.exe"
mkdir -p dist
cp "$exe" dist/pomodoro-soundscapes-windows-x64.exe

# WiX Toolset v3 (candle/light). Use the installed copy, or install it with Chocolatey.
find_wix() {
  local dir
  for dir in "${WIX:-}" /c/Program\ Files\ \(x86\)/WiX\ Toolset*; do
    if [ -n "$dir" ] && [ -f "$dir/bin/candle.exe" ]; then
      echo "$dir/bin"
      return 0
    fi
  done
  return 1
}
if ! bin="$(find_wix)"; then
  choco install wixtoolset --no-progress -y
  bin="$(find_wix)"
fi

obj="dist/wix-obj"
mkdir -p "$obj"
"$bin/candle.exe" -nologo -arch x64 -dVersion="$version" -dExePath="$exe" \
  -out "$obj/" packaging/windows/pomodoro.wxs
"$bin/light.exe" -nologo -out dist/pomodoro-soundscapes-windows-x64.msi "$obj/pomodoro.wixobj"
rm -rf "$obj" dist/*.wixpdb
