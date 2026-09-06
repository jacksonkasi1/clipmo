#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
test_dir=$(mktemp -d)
trap 'rm -rf "$test_dir"' EXIT
xcrun clang -fobjc-arc -fblocks -framework AppKit -framework ApplicationServices \
  src-tauri/src/macos/native.m scripts/test-macos-native.m -o "$test_dir/native-test"
"$test_dir/native-test"
