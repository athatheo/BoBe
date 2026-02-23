#!/usr/bin/env bash
# Build BoBe.app DMG for macOS ARM
#
# Runs the full build pipeline:
# 1. Build Python service bundle (from sibling repo or CI clone)
# 2. Build Electron + React app
# 3. Package into .app + .dmg via electron-builder
#
# Output: dist/BoBe-*.dmg
#
# Usage:
#   ./packaging/build-dmg.sh                                # local dev
#   SERVICE_DIR=../service ./packaging/build-dmg.sh         # custom service path
#   CI=true SERVICE_REPO_URL=... ./packaging/build-dmg.sh   # CI
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DESKTOP_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==> Building BoBe DMG"

# Step 1: Build Python service bundle
echo ""
echo "=== Step 1/3: Service Bundle ==="
"$SCRIPT_DIR/build-service-bundle.sh"

# Step 2: Build Electron + React
echo ""
echo "=== Step 2/3: Electron Build ==="
cd "$DESKTOP_ROOT"
pnpm install --frozen-lockfile
pnpm build

# Step 3: Package with electron-builder
echo ""
echo "=== Step 3/3: electron-builder ==="
pnpm exec electron-builder --mac --arm64 --publish never

echo ""
echo "==> Build complete!"
ls -lh "$DESKTOP_ROOT/dist/"*.dmg 2>/dev/null || echo "    (no .dmg found — check for errors above)"
