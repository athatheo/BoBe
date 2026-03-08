# Default recipe
default:
    @just --list

# Build Rust backend (release)
build-backend:
    cargo build --release

# Build Swift frontend (release)
build-frontend:
    cd desktopMac && swift build -c release

# Assemble .app bundle
bundle version="1.0.0":
    #!/bin/bash
    set -euo pipefail
    APP="build/BoBe.app"
    rm -rf "$APP"
    mkdir -p "$APP/Contents/MacOS"
    mkdir -p "$APP/Contents/Resources"

    # Copy binaries (ditto preserves symlinks per Apple docs)
    # Backend named "bobe-daemon" to avoid case-insensitive collision with "BoBe" on APFS
    ditto target/release/bobe "$APP/Contents/MacOS/bobe-daemon"
    ditto desktopMac/.build/release/BoBe "$APP/Contents/MacOS/BoBe"

    # Copy Info.plist and update version
    cp desktopMac/BoBe/Resources/Info.plist "$APP/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleVersion {{ version }}" "$APP/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString {{ version }}" "$APP/Contents/Info.plist"

    # Copy all resources (icons, images)
    cp -r desktopMac/BoBe/Resources/ "$APP/Contents/Resources/" 2>/dev/null || true
    rm -f "$APP/Contents/Resources/Info.plist"

    # Strip debug symbols for smaller binary
    strip -x "$APP/Contents/MacOS/bobe-daemon" 2>/dev/null || true
    strip -x "$APP/Contents/MacOS/BoBe" 2>/dev/null || true

    echo "Bundle created at $APP"
    echo "  Backend: $(wc -c < "$APP/Contents/MacOS/bobe-daemon" | tr -d ' ') bytes"
    echo "  Frontend: $(wc -c < "$APP/Contents/MacOS/BoBe" | tr -d ' ') bytes"

# Full build + bundle
build version="1.0.0": build-backend build-frontend (bundle version)

# Sign the bundle — inside-out order per Apple docs
sign identity="Developer ID Application":
    #!/bin/bash
    set -euo pipefail
    APP="build/BoBe.app"

    # 1. Sign the embedded backend binary first (non-bundled main executable)
    codesign -s "{{ identity }}" --options runtime --timestamp \
        --entitlements desktopMac/entitlements.plist \
        "$APP/Contents/MacOS/bobe-daemon"

    # 2. Sign the app bundle (signs the frontend binary + seals the bundle)
    codesign -s "{{ identity }}" --options runtime --timestamp --force \
        --entitlements desktopMac/entitlements.plist \
        "$APP"

    # 3. Verify
    codesign --verify --deep --strict "$APP"
    echo "Signing verified successfully"

# Create signed DMG
dmg version="1.0.0": (build version)
    #!/bin/bash
    set -euo pipefail
    DMG_DIR="build/dmg-staging"
    DMG_NAME="BoBe-{{ version }}.dmg"
    rm -rf "$DMG_DIR" "build/$DMG_NAME"
    mkdir -p "$DMG_DIR"

    ditto build/BoBe.app "$DMG_DIR/BoBe.app"
    ln -s /Applications "$DMG_DIR/Applications"

    hdiutil create -volname "BoBe {{ version }}" \
        -srcfolder "$DMG_DIR" \
        -ov -format UDZO \
        "build/$DMG_NAME"

    rm -rf "$DMG_DIR"
    echo "DMG created: build/$DMG_NAME"
    ls -lh "build/$DMG_NAME"

# Sign the DMG (after signing the app inside it)
sign-dmg identity="Developer ID Application" version="1.0.0":
    codesign -s "{{ identity }}" --timestamp \
        -i com.bobe.app.dmg \
        "build/BoBe-{{ version }}.dmg"
    echo "DMG signed"

# Notarize (requires Apple ID credentials)
notarize version="1.0.0" apple-id="" team-id="" password="":
    xcrun notarytool submit "build/BoBe-{{ version }}.dmg" \
        --apple-id "{{ apple-id }}" \
        --team-id "{{ team-id }}" \
        --password "{{ password }}" \
        --wait
    echo "Notarization complete"

# Notarize using an App Store Connect API key (CI-friendly)
notarize-api-key version="1.0.0" key-path="" key-id="" issuer="":
    #!/bin/bash
    set -euo pipefail
    if [[ -z "{{ key-path }}" || -z "{{ key-id }}" || -z "{{ issuer }}" ]]; then
        echo "notarize-api-key requires key-path, key-id, and issuer" >&2
        exit 1
    fi
    xcrun notarytool submit "build/BoBe-{{ version }}.dmg" \
        --key "{{ key-path }}" \
        --key-id "{{ key-id }}" \
        --issuer "{{ issuer }}" \
        --wait
    echo "Notarization complete"

# Staple the notarization ticket
staple version="1.0.0":
    xcrun stapler staple "build/BoBe-{{ version }}.dmg"
    echo "Stapled successfully"

# Full release: build → sign app → create DMG → sign DMG
release version="1.0.0" identity="Developer ID Application": (build version) (sign identity) (dmg version) (sign-dmg identity version)
    echo "Release build complete: build/BoBe-{{ version }}.dmg"
    echo "Next: just notarize {{ version }} apple-id=... team-id=... password=..."
    echo "   or: just notarize-api-key {{ version }} key-path=... key-id=... issuer=..."
    echo "Then: just staple {{ version }}"

# Create Sparkle-friendly ZIP archive of the signed app bundle
sparkle-zip version="1.0.0":
    ditto -c -k --sequesterRsrc --keepParent build/BoBe.app "build/BoBe-{{ version }}.zip"
    echo "Sparkle archive created: build/BoBe-{{ version }}.zip"

# Sign Sparkle update archive (prints enclosure attributes)
sparkle-sign-update version="1.0.0" private-key-file="":
    #!/bin/bash
    set -euo pipefail
    SPARKLE_BIN="desktopMac/.build/artifacts/sparkle/Sparkle/bin"
    if [[ ! -x "$SPARKLE_BIN/sign_update" ]]; then
        echo "Sparkle tools not found. Run: cd desktopMac && swift package resolve"
        exit 1
    fi
    ARGS=()
    if [[ -n "{{ private-key-file }}" ]]; then
        ARGS+=(-f "{{ private-key-file }}")
    fi
    "$SPARKLE_BIN/sign_update" "${ARGS[@]}" "build/BoBe-{{ version }}.zip"

# Generate/update appcast.xml from staged Sparkle archives
sparkle-generate-appcast archives_dir="build/sparkle" download_url_prefix="" link="":
    #!/bin/bash
    set -euo pipefail
    SPARKLE_BIN="desktopMac/.build/artifacts/sparkle/Sparkle/bin"
    if [[ ! -x "$SPARKLE_BIN/generate_appcast" ]]; then
        echo "Sparkle tools not found. Run: cd desktopMac && swift package resolve"
        exit 1
    fi
    mkdir -p "{{ archives_dir }}"
    ARGS=()
    if [[ -n "{{ download_url_prefix }}" ]]; then
        ARGS+=(--download-url-prefix "{{ download_url_prefix }}")
    fi
    if [[ -n "{{ link }}" ]]; then
        ARGS+=(--link "{{ link }}")
    fi
    "$SPARKLE_BIN/generate_appcast" "${ARGS[@]}" "{{ archives_dir }}"
    echo "Generated appcast at {{ archives_dir }}/appcast.xml"

# Build debug + launch app (Swift app manages backend lifecycle)
run:
    #!/bin/bash
    set -euo pipefail
    cargo build
    (cd desktopMac && swift build -c debug)
    # Place backend where BackendService.findBinaryPath() discovers it
    mkdir -p desktopMac/.build/debug
    cp target/debug/bobe desktopMac/.build/debug/bobe-daemon
    echo "Launching BoBe..."
    desktopMac/.build/debug/BoBe

# Run backend only (use when running frontend from Xcode)
backend:
    cargo run -- serve

# Alias for backend
run-backend: backend

# Clean all build artifacts
clean:
    cargo clean --workspace
    cd desktopMac && swift package clean
    rm -rf build/

# Generate Xcode project (for previews/debugging)
xcode:
    cd desktopMac && xcodegen generate

# Format Swift frontend source files
format-swift:
    cd desktopMac && swiftformat BoBe

# Check Swift frontend formatting
check-swift-format:
    cd desktopMac && swiftformat --lint BoBe

# fmt + clippy + test + deny + machete + swiftlint + swift build
check:
    cargo fmt --check
    cargo clippy -q
    cargo test -q
    cargo deny check
    cargo machete
    cd desktopMac && swiftlint lint --quiet
    cd desktopMac && swift build -c debug

# CI vetting: deterministic Rust + supply-chain + Swift
check-ci:
    cargo fmt --check
    cargo clippy --locked -q
    cargo test --locked -q
    cargo deny check
    cargo vet --locked
    cargo machete
    cd desktopMac && swiftlint lint --quiet
    cd desktopMac && swift build -c debug

# Alias for check (muscle memory)
test: check

# Full ship: clean → resolve deps → build → sign → DMG → notarize → staple → Sparkle zip
ship version apple-id team-id password identity="Developer ID Application":
    #!/bin/bash
    set -euo pipefail
    echo "=== Clean ==="
    just clean

    echo "=== Resolve dependencies ==="
    cargo fetch
    (cd desktopMac && swift package resolve)

    echo "=== Build + Bundle + Sign + DMG ==="
    just release {{ version }} {{ identity }}

    echo "=== Notarize ==="
    just notarize {{ version }} apple-id={{ apple-id }} team-id={{ team-id }} password={{ password }}

    echo "=== Staple ==="
    just staple {{ version }}

    echo "=== Sparkle ZIP ==="
    just sparkle-zip {{ version }}

    echo ""
    echo "=== Ship complete ==="
    echo "DMG:         build/BoBe-{{ version }}.dmg (signed + notarized + stapled)"
    echo "Sparkle ZIP: build/BoBe-{{ version }}.zip"
    echo ""
    echo "Next: just sparkle-sign-update {{ version }}"
    echo "Then: upload DMG + ZIP + update appcast.xml"
