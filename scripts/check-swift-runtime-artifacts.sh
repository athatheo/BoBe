#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONFIGURATION="${1:-debug}"
BIN_DIR="$(cd "$ROOT/BoBeMacUI" && swift build -c "$CONFIGURATION" --show-bin-path)"
EXECUTABLE="$BIN_DIR/BoBe"

test -x "$EXECUTABLE"
test -f "$BIN_DIR/Sparkle.framework/Versions/B/Sparkle"
test -d "$BIN_DIR/BoBe_BoBe.bundle"
test -d "$BIN_DIR/textual_Textual.bundle"
test -d "$BIN_DIR/swiftui-math_SwiftUIMath.bundle"

otool -L "$EXECUTABLE" | grep -q '@rpath/Sparkle.framework/Versions/B/Sparkle'
otool -l "$EXECUTABLE" | grep -q '@executable_path/../Frameworks'

echo "Swift runtime artifacts ok ($CONFIGURATION)"
