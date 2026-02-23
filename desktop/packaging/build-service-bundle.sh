#!/usr/bin/env bash
# Build a relocatable Python bundle containing the BoBe service.
#
# Two modes:
#   Local (default): builds from a sibling checkout of the service repo.
#     SERVICE_DIR=../ProactiveAi  (default, or override with any path)
#   CI:              clones from a remote URL.
#     CI=true SERVICE_REPO_URL=git@github.com:athatheo/ProactiveAI.git
#
# Output: packaging/.build/python-bundle/
#
# Usage:
#   ./packaging/build-service-bundle.sh                         # local, default sibling
#   SERVICE_DIR=../service ./packaging/build-service-bundle.sh  # local, custom path
#   CI=true SERVICE_REPO_URL=... ./packaging/build-service-bundle.sh  # CI
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DESKTOP_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Configuration -----------------------------------------------------------
PYTHON_VERSION="3.13.12"
PBS_RELEASE="20260203"
ARCH="aarch64-apple-darwin"
VARIANT="install_only_stripped"
TARBALL="cpython-${PYTHON_VERSION}+${PBS_RELEASE}-${ARCH}-${VARIANT}.tar.gz"
DOWNLOAD_URL="https://github.com/astral-sh/python-build-standalone/releases/download/${PBS_RELEASE}/${TARBALL}"

CACHE_DIR="${SCRIPT_DIR}/.cache"
BUILD_DIR="${SCRIPT_DIR}/.build"
BUNDLE_DIR="${BUILD_DIR}/python-bundle"
CHECKSUMS_FILE="${SCRIPT_DIR}/checksums.txt"

# --- Resolve service source ---------------------------------------------------
if [[ "${CI:-}" == "true" ]]; then
    SERVICE_REPO_URL="${SERVICE_REPO_URL:?SERVICE_REPO_URL is required in CI mode}"
    SERVICE_BRANCH="${SERVICE_BRANCH:-main}"
    echo "==> CI mode: cloning ${SERVICE_REPO_URL} (branch: ${SERVICE_BRANCH})"

    CLONE_DIR="${CACHE_DIR}/service-src"
    rm -rf "$CLONE_DIR"
    git clone --depth 1 --branch "$SERVICE_BRANCH" "$SERVICE_REPO_URL" "$CLONE_DIR"
    SERVICE_DIR="$CLONE_DIR"
else
    SERVICE_DIR="${SERVICE_DIR:-$(cd "$DESKTOP_ROOT/.." && pwd)/ProactiveAi}"
    if [[ ! -d "$SERVICE_DIR" ]]; then
        echo "ERROR: Service directory not found at ${SERVICE_DIR}"
        echo ""
        echo "Expected a sibling checkout of the service repo."
        echo "Override with:  SERVICE_DIR=/path/to/service $0"
        exit 1
    fi
    SERVICE_DIR="$(cd "$SERVICE_DIR" && pwd)"
    echo "==> Local mode: using service at ${SERVICE_DIR}"
fi

echo "==> Building Python bundle for BoBe"
echo "    Python: ${PYTHON_VERSION}"
echo "    Arch:   ${ARCH}"
echo "    Service: ${SERVICE_DIR}"

# --- Step 1: Download (cached) -----------------------------------------------
mkdir -p "$CACHE_DIR"
if [[ ! -f "${CACHE_DIR}/${TARBALL}" ]]; then
    echo "==> Downloading python-build-standalone..."
    curl -L --progress-bar -o "${CACHE_DIR}/${TARBALL}" "$DOWNLOAD_URL"
else
    echo "==> Using cached ${TARBALL}"
fi

# --- Step 2: Verify checksum -------------------------------------------------
echo "==> Verifying SHA256 checksum..."
cd "$CACHE_DIR"
grep -F "$TARBALL" "$CHECKSUMS_FILE" | shasum -a 256 -c -
cd "$DESKTOP_ROOT"

# --- Step 3: Extract ----------------------------------------------------------
echo "==> Extracting to ${BUNDLE_DIR}..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"
tar -xzf "${CACHE_DIR}/${TARBALL}" -C "$BUNDLE_DIR" --strip-components=1

# --- Step 4: Install service --------------------------------------------------
echo "==> Installing bobe service into bundle..."
"${BUNDLE_DIR}/bin/pip" install --no-cache-dir "${SERVICE_DIR}"

# --- Step 5: Strip unnecessary files -----------------------------------------
echo "==> Stripping unnecessary files..."
find "$BUNDLE_DIR" -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
find "$BUNDLE_DIR" -type d -name "tests" -path "*/site-packages/*/tests" -exec rm -rf {} + 2>/dev/null || true
find "$BUNDLE_DIR" -type d -name "test" -path "*/site-packages/*/test" -exec rm -rf {} + 2>/dev/null || true
find "$BUNDLE_DIR" -name "*.pyc" -delete 2>/dev/null || true
"${BUNDLE_DIR}/bin/pip" uninstall -y pip setuptools 2>/dev/null || true

# --- Step 6: Fix dylib relocatability ----------------------------------------
echo "==> Fixing dylib install names for relocatability..."
LIBPYTHON=$(find "$BUNDLE_DIR" -name "libpython*.dylib" -print -quit)
if [[ -n "$LIBPYTHON" ]]; then
    LIBPYTHON_NAME=$(basename "$LIBPYTHON")
    echo "    Fixing ${LIBPYTHON_NAME}"

    # Fix the dylib's own identity (remove absolute build path)
    install_name_tool -id "@executable_path/../lib/${LIBPYTHON_NAME}" "$LIBPYTHON"

    # Fix the python3 binary's reference to libpython
    PYTHON_BIN="${BUNDLE_DIR}/bin/python3"
    if [[ -f "$PYTHON_BIN" ]]; then
        # Get the current (absolute) install name from the binary
        OLD_NAME=$(otool -L "$PYTHON_BIN" | grep "libpython" | head -1 | awk '{print $1}')
        if [[ -n "$OLD_NAME" && "$OLD_NAME" != "@"* ]]; then
            echo "    Fixing python3 reference: ${OLD_NAME} -> @executable_path/../lib/${LIBPYTHON_NAME}"
            install_name_tool -change "$OLD_NAME" "@executable_path/../lib/${LIBPYTHON_NAME}" "$PYTHON_BIN"
        fi
    fi

    # Fix references in .so extension modules that link libpython.
    # .so files live at varying depths (lib-dynload, site-packages/*/...),
    # so compute the relative path from each .so to the lib/ directory.
    LIB_DIR=$(dirname "$LIBPYTHON")
    while IFS= read -r -d '' SO_FILE; do
        OLD_REF=$(otool -L "$SO_FILE" | grep "libpython" | head -1 | awk '{print $1}')
        if [[ -n "$OLD_REF" && "$OLD_REF" != "@"* ]]; then
            SO_DIR=$(dirname "$SO_FILE")
            # Use Python from the bundle itself to compute the relative path
            REL_PATH=$("${BUNDLE_DIR}/bin/python3" -c "import os; print(os.path.relpath('${LIB_DIR}', '${SO_DIR}'))")
            install_name_tool -change "$OLD_REF" "@loader_path/${REL_PATH}/${LIBPYTHON_NAME}" "$SO_FILE"
        fi
    done < <(find "$BUNDLE_DIR" -name "*.so" -print0)

    # Verify: no absolute paths should remain in libpython
    echo "==> Verifying relocatability..."
    if otool -L "$LIBPYTHON" | grep -v "@" | grep -q "/install/"; then
        echo "ERROR: Absolute paths still present in ${LIBPYTHON_NAME}:"
        otool -L "$LIBPYTHON"
        exit 1
    fi
    echo "    Relocatability verified"
else
    echo "    WARNING: No libpython dylib found — skipping install_name_tool fixes"
fi

# --- Step 6b: Fix third-party dylib install names ---------------------------
# Packages like torch, av, PIL, scipy, scikit-learn, ctranslate2 ship .dylibs
# with build-artifact install names (/DLC/..., /opt/homebrew/..., dist/...).
# These work at runtime (consumers use @rpath/@loader_path), but non-relocatable
# install names can cause notarization warnings and are incorrect for distribution.
echo "==> Fixing third-party dylib install names..."
FIXED_DYLIBS=0
while IFS= read -r -d '' DYLIB; do
    # Skip libpython (already handled above)
    case "$DYLIB" in */libpython*) continue ;; esac

    INSTALL_NAME=$(otool -D "$DYLIB" | tail -1)
    # Fix any install name that isn't already @rpath/@loader_path/@executable_path
    if [[ -n "$INSTALL_NAME" && "$INSTALL_NAME" != "@"* ]]; then
        DYLIB_BASENAME=$(basename "$DYLIB")
        install_name_tool -id "@loader_path/${DYLIB_BASENAME}" "$DYLIB"
        FIXED_DYLIBS=$((FIXED_DYLIBS + 1))
    fi
done < <(find "$BUNDLE_DIR" -name "*.dylib" -print0)
echo "    Fixed install names on ${FIXED_DYLIBS} third-party dylibs"

# --- Step 6c: Deduplicate libomp.dylib (torch + scikit-learn) ---------------
# Both torch and scikit-learn bundle their own libomp.dylib. Loading both into
# the same process causes: "OMP: Error #15: Initializing libiomp5, but found
# libomp already initialized." Replace sklearn's copy with a symlink to torch's.
TORCH_OMP=$(find "$BUNDLE_DIR" -path "*/torch/lib/libomp.dylib" -print -quit)
SKLEARN_OMP=$(find "$BUNDLE_DIR" -path "*/sklearn/.dylibs/libomp.dylib" -print -quit)
if [[ -n "$TORCH_OMP" && -n "$SKLEARN_OMP" ]]; then
    echo "==> Deduplicating libomp.dylib (torch + scikit-learn)"
    # Compute relative path from sklearn/.dylibs/ to torch/lib/
    SKLEARN_DIR=$(dirname "$SKLEARN_OMP")
    TORCH_DIR=$(dirname "$TORCH_OMP")
    REL_OMP=$("${BUNDLE_DIR}/bin/python3" -c "import os; print(os.path.relpath('${TORCH_OMP}', '${SKLEARN_DIR}'))")
    rm "$SKLEARN_OMP"
    ln -s "$REL_OMP" "$SKLEARN_OMP"
    echo "    Replaced sklearn libomp.dylib with symlink -> ${REL_OMP}"
elif [[ -n "$TORCH_OMP" || -n "$SKLEARN_OMP" ]]; then
    echo "    Only one libomp.dylib found — no dedup needed"
else
    echo "    No libomp.dylib found — skipping dedup"
fi

# --- Step 7: Smoke test ------------------------------------------------------
echo "==> Running smoke test..."
"${BUNDLE_DIR}/bin/python3" -c "from bobe.cli import app; print('Smoke test passed: bobe CLI importable')"

BUNDLE_SIZE=$(du -sh "$BUNDLE_DIR" | cut -f1)
echo "==> Python bundle ready at ${BUNDLE_DIR} (${BUNDLE_SIZE})"
echo "    To test: ${BUNDLE_DIR}/bin/python3 -m bobe --version"
