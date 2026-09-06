#!/bin/bash
set -euo pipefail
target=${1:?Rust target required}
cd "$(dirname "$0")/.."
bundle="src-tauri/target/$target/release/bundle"
app="$bundle/macos/Clipmo.app"
binary="$app/Contents/MacOS/clipmo"
case "$target" in
  aarch64-apple-darwin) architecture=arm64 ;;
  x86_64-apple-darwin) architecture=x86_64 ;;
  *) echo "Unsupported target: $target" >&2; exit 1 ;;
esac
test -x "$binary"
lipo -verify_arch "$architecture" "$binary"
codesign --verify --deep --strict --verbose=2 "$app"
version=$(node -p 'JSON.parse(require("fs").readFileSync("package.json")).version')
test "$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$app/Contents/Info.plist")" = "$version"
mkdir -p artifacts/macos
shopt -s nullglob
dmgs=("$bundle"/dmg/*.dmg)
test "${#dmgs[@]}" -eq 1
hdiutil verify "${dmgs[0]}"
cp "${dmgs[0]}" "artifacts/macos/Clipmo_${version}_${architecture}.dmg"
ditto -c -k --sequesterRsrc --keepParent "$app" "artifacts/macos/Clipmo_${version}_${architecture}.app.zip"
(cd artifacts/macos && shasum -a 256 "Clipmo_${version}_${architecture}.dmg" "Clipmo_${version}_${architecture}.app.zip" > "Clipmo_${version}_${architecture}.sha256")
