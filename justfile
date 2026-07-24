# Default recipe
default:
    @just --list

copilot_cli_dir := justfile_directory() + "/build/copilot-cli/runtime"
copilot_cli_cache_dir := justfile_directory() + "/.tools/cache/copilot-cli"

# Build Rust backend and stage the SDK-pinned Copilot CLI helper.
build-backend profile="release":
    #!/bin/bash
    set -euo pipefail
    profile="{{ profile }}"
    cli_dir="{{ copilot_cli_dir }}"
    cli="$cli_dir/copilot"
    stamp="$cli_dir/.cargo-lock.sha256"
    lock_hash="$(shasum -a 256 BoBeService/Cargo.lock | awk '{print $1}')"
    stamp_lock_hash=""
    stamp_helper_hash=""
    if [[ -f "$stamp" ]]; then
        read -r stamp_lock_hash stamp_helper_hash <"$stamp" || true
    fi
    helper_sha256() {
        shasum -a 256 "$cli" | awk '{print $1}'
    }
    helper_is_valid() {
        local signature version
        [[ -x "$cli" ]] || return 1
        codesign --verify --strict "$cli" >/dev/null 2>&1 || return 1
        signature="$(codesign -d --verbose=2 "$cli" 2>&1)" || return 1
        [[ "$signature" == *"TeamIdentifier=VEKTX9H2N7"* ]] || return 1
        # The SDK asset tag and CLI's reported internal version can differ.
        version="$("$cli" --version)" || return 1
        [[ "$version" == *"GitHub Copilot CLI "* ]]
    }
    if [[ "$stamp_lock_hash" != "$lock_hash" ]] ||
        ! helper_is_valid ||
        [[ "$stamp_helper_hash" != "$(helper_sha256)" ]]; then
        rm -f "$cli" "$stamp"
    fi
    case "$profile" in
        release) profile_args=(--release) ;;
        debug) profile_args=(--profile dev) ;;
        *)
            echo "build-backend profile must be 'debug' or 'release': $profile" >&2
            exit 1
            ;;
    esac
    (
        cd BoBeService
        COPILOT_CLI_EXTRACT_DIR="$cli_dir" \
            BUNDLED_CLI_CACHE_DIR="{{ copilot_cli_cache_dir }}" \
            cargo build --locked --no-default-features "${profile_args[@]}"
    )
    helper_is_valid || {
        echo "SDK-staged Copilot CLI helper failed provenance validation: $cli" >&2
        exit 1
    }
    helper_hash="$(helper_sha256)"
    printf '%s %s\n' "$lock_hash" "$helper_hash" >"$stamp.tmp"
    mv "$stamp.tmp" "$stamp"

# Build the SwiftPM executable used by the real-adapter smoke
build-frontend:
    cd BoBeMacUI && swift build -c release

# Build the native app layout so package resource bundles land in
# Contents/Resources instead of the codesign-invalid .app root.
build-app-frontend: xcode
    cd BoBeMacUI && xcodebuild \
        -project BoBe.xcodeproj \
        -scheme BoBe \
        -configuration Release \
        -destination 'generic/platform=macOS' \
        -derivedDataPath .build/xcode-derived \
        -onlyUsePackageVersionsFromResolvedFile \
        CODE_SIGNING_ALLOWED=NO \
        build -quiet

# Measure listener readiness, Copilot prewarm, RSS, and release binary size
profile-startup: build-backend
    COPILOT_CLI_PATH="{{ copilot_cli_dir }}/copilot" ./scripts/profile-daemon-startup.sh

# Opt-in real Copilot smoke (requires authentication/network and uses AI credits)
smoke-copilot-tools: build-backend
    COPILOT_CLI_PATH="{{ copilot_cli_dir }}/copilot" ./scripts/smoke-copilot-tools.sh

# Opt-in real BodyLink smoke with mTLS body + correlated adapter simulators
check-bodylink-contract:
    ./scripts/check-bodylink-contract.sh

smoke-bodylink: build-backend check-bodylink-contract
    COPILOT_CLI_PATH="{{ copilot_cli_dir }}/copilot" ./scripts/smoke-bodylink.sh

# BodyLink smoke through the real Swift FluidAudio/Opus adapter
smoke-bodylink-swift: build-backend build-frontend check-bodylink-contract
    COPILOT_CLI_PATH="{{ copilot_cli_dir }}/copilot" \
        BOBE_BODY_SMOKE_REAL_ADAPTER=1 ./scripts/smoke-bodylink.sh

# Assemble .app bundle
bundle version="1.0.0":
    #!/bin/bash
    set -euo pipefail
    APP="build/BoBe.app"
    XCODE_APP="BoBeMacUI/.build/xcode-derived/Build/Products/Release/BoBe.app"
    test -d "$XCODE_APP" || {
        echo "Xcode release app not found: $XCODE_APP" >&2
        exit 1
    }
    rm -rf "$APP"
    ditto "$XCODE_APP" "$APP"

    COPILOT_CLI="{{ copilot_cli_dir }}/copilot"
    test -x "$COPILOT_CLI" || {
        echo "Staged Copilot CLI helper not found: $COPILOT_CLI" >&2
        exit 1
    }
    mkdir -p "$APP/Contents/Helpers"
    ditto "$COPILOT_CLI" "$APP/Contents/Helpers/copilot"

    # Backend named "bobe-daemon" to avoid a case-insensitive collision with
    # the frontend executable on APFS.
    ditto BoBeService/target/release/bobe "$APP/Contents/MacOS/bobe-daemon"

    # Update the Xcode-produced Info.plist with the release version.
    /usr/libexec/PlistBuddy -c "Set :CFBundleVersion {{ version }}" "$APP/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString {{ version }}" "$APP/Contents/Info.plist"

    # Strip debug symbols for smaller binary
    strip -x "$APP/Contents/MacOS/bobe-daemon" 2>/dev/null || true
    strip -x "$APP/Contents/MacOS/BoBe" 2>/dev/null || true

    # Fail assembly if any linked @rpath framework was not embedded.
    while IFS= read -r dependency; do
        framework=${dependency#@rpath/}
        framework=${framework%%/Versions/*}
        test -d "$APP/Contents/Frameworks/$framework" || {
            echo "Missing embedded framework: $framework" >&2
            exit 1
        }
    done < <(otool -L "$APP/Contents/MacOS/BoBe" | grep -o '@rpath/[^ ]*\.framework/[^ ]*' || true)
    for resource_bundle in textual_Textual.bundle swiftui-math_SwiftUIMath.bundle; do
        test -d "$APP/Contents/Resources/$resource_bundle" || {
            echo "Missing SwiftPM resource bundle: $resource_bundle" >&2
            exit 1
        }
    done
    test -f "$APP/Contents/Resources/en.lproj/UI.strings" || {
        echo "Missing BoBe localization resources" >&2
        exit 1
    }

    echo "Bundle created at $APP"
    echo "  Backend: $(wc -c < "$APP/Contents/MacOS/bobe-daemon" | tr -d ' ') bytes"
    echo "  Copilot: $(wc -c < "$APP/Contents/Helpers/copilot" | tr -d ' ') bytes"
    echo "  Frontend: $(wc -c < "$APP/Contents/MacOS/BoBe" | tr -d ' ') bytes"

# Full build + bundle + local ad-hoc signature
build version="1.0.0": build-backend build-app-frontend (bundle version)
    just sign -

# Sign the bundle — inside-out order per Apple docs
sign identity="Developer ID Application":
    #!/bin/bash
    set -euo pipefail
    APP="build/BoBe.app"
    SPARKLE="$APP/Contents/Frameworks/Sparkle.framework"
    SPARKLE_VERSION="$SPARKLE/Versions/B"
    identity="{{ identity }}"
    SIGN_ARGS=(--force --sign "$identity")
    if [[ "$identity" != "-" ]]; then
        SIGN_ARGS+=(--options runtime --timestamp)
    fi

    # 1. Sign nested helper executables before the binaries that contain them.
    codesign "${SIGN_ARGS[@]}" \
        --entitlements BoBeMacUI/copilot-helper.entitlements.plist \
        "$APP/Contents/Helpers/copilot"

    # 2. Sign the embedded backend binary.
    codesign "${SIGN_ARGS[@]}" \
        --entitlements BoBeMacUI/entitlements.plist \
        "$APP/Contents/MacOS/bobe-daemon"

    # 3. Sparkle's helpers must be signed explicitly inside-out. Do not use
    # --deep: Downloader.xpc has distinct entitlements that must be preserved.
    if [[ -d "$SPARKLE" ]]; then
        codesign "${SIGN_ARGS[@]}" \
            "$SPARKLE_VERSION/XPCServices/Installer.xpc"
        if [[ -d "$SPARKLE_VERSION/XPCServices/Downloader.xpc" ]]; then
            codesign "${SIGN_ARGS[@]}" \
                --preserve-metadata=entitlements \
                "$SPARKLE_VERSION/XPCServices/Downloader.xpc"
        fi
        codesign "${SIGN_ARGS[@]}" \
            "$SPARKLE_VERSION/Autoupdate"
        codesign "${SIGN_ARGS[@]}" \
            "$SPARKLE_VERSION/Updater.app"
        codesign "${SIGN_ARGS[@]}" \
            "$SPARKLE"
    fi

    # 4. Sign any other embedded dynamic frameworks.
    for framework in "$APP/Contents/Frameworks"/*.framework; do
        [[ -d "$framework" && "$framework" != "$SPARKLE" ]] || continue
        codesign "${SIGN_ARGS[@]}" "$framework"
    done

    # 5. Sign the app bundle (signs the frontend binary + seals the bundle)
    codesign "${SIGN_ARGS[@]}" \
        --entitlements BoBeMacUI/entitlements.plist \
        "$APP"

    # 6. Verify
    codesign --verify --deep --strict --verbose=2 "$APP"
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

# Notarize with credentials stored by notarytool in the macOS Keychain.
# Requires: NOTARIZE_KEYCHAIN_PROFILE
notarize version="1.0.0":
    #!/bin/bash
    set -euo pipefail
    : "${NOTARIZE_KEYCHAIN_PROFILE:?Set NOTARIZE_KEYCHAIN_PROFILE to a notarytool Keychain profile}"
    xcrun notarytool submit "build/BoBe-{{ version }}.dmg" \
        --keychain-profile "$NOTARIZE_KEYCHAIN_PROFILE" \
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

# Staple and validate the notarization ticket on the app before packaging updates
staple version="1.0.0":
    xcrun stapler staple build/BoBe.app
    xcrun stapler validate build/BoBe.app
    xcrun stapler staple "build/BoBe-{{ version }}.dmg"
    xcrun stapler validate "build/BoBe-{{ version }}.dmg"
    echo "App and DMG stapled and validated successfully"

# Full release: build → sign app → create DMG → sign DMG
release version="1.0.0" identity="Developer ID Application": (build version) (sign identity) (dmg version) (sign-dmg identity version)
    echo "Release build complete: build/BoBe-{{ version }}.dmg"
    echo "Next: set NOTARIZE_KEYCHAIN_PROFILE to a notarytool Keychain profile, then run: just notarize {{ version }}"
    echo "   or: just notarize-api-key {{ version }} /path/to/AuthKey.p8 KEY_ID ISSUER_ID"
    echo "Then: just staple {{ version }}"

# Create Sparkle-friendly ZIP archive of the signed app bundle
sparkle-zip version="1.0.0":
    ditto -c -k --sequesterRsrc --keepParent build/BoBe.app "build/BoBe-{{ version }}.zip"
    echo "Sparkle archive created: build/BoBe-{{ version }}.zip"

# Sign Sparkle update archive (prints enclosure attributes)
sparkle-sign-update version="1.0.0" private-key-file="":
    #!/bin/bash
    set -euo pipefail
    normalize_named_arg() {
        local parameter_name="$1"
        local value="$2"
        if [[ "$value" == "${parameter_name}="* ]]; then
            printf '%s\n' "${value#"${parameter_name}="}"
            return
        fi
        printf '%s\n' "$value"
    }
    version="$(normalize_named_arg "version" "{{ version }}")"
    private_key_file="$(normalize_named_arg "private-key-file" "{{ private-key-file }}")"
    SPARKLE_BIN="BoBeMacUI/.build/artifacts/sparkle/Sparkle/bin"
    if [[ ! -x "$SPARKLE_BIN/sign_update" ]]; then
        echo "Sparkle tools not found. Run: cd BoBeMacUI && swift package resolve"
        exit 1
    fi
    archive="build/BoBe-$version.zip"
    if [[ ! -f "$archive" ]]; then
        echo "Sparkle archive not found: $archive. Run: just sparkle-zip $version"
        exit 1
    fi
    ARGS=()
    if [[ -n "$private_key_file" ]]; then
        if [[ "$private_key_file" != "-" && ! -f "$private_key_file" ]]; then
            echo "Sparkle private key file not found: $private_key_file"
            exit 1
        fi
        ARGS+=(-f "$private_key_file")
    fi
    "$SPARKLE_BIN/sign_update" "${ARGS[@]}" "$archive"

# Generate/update appcast.xml from staged Sparkle archives
sparkle-generate-appcast archives_dir="build/sparkle" download_url_prefix="" ed-key-file="" link="" ed-dsa-key-file="":
    #!/bin/bash
    set -euo pipefail
    normalize_named_arg() {
        local parameter_name="$1"
        local value="$2"
        if [[ "$value" == "${parameter_name}="* ]]; then
            printf '%s\n' "${value#"${parameter_name}="}"
            return
        fi
        printf '%s\n' "$value"
    }
    archives_dir="$(normalize_named_arg "archives_dir" "{{ archives_dir }}")"
    download_url_prefix="$(normalize_named_arg "download_url_prefix" "{{ download_url_prefix }}")"
    ed_key_file="$(normalize_named_arg "ed-key-file" "{{ ed-key-file }}")"
    ed_key_file="$(normalize_named_arg "ed-dsa-key-file" "$ed_key_file")"
    legacy_ed_dsa_key_file="$(normalize_named_arg "ed-dsa-key-file" "{{ ed-dsa-key-file }}")"
    legacy_ed_dsa_key_file="$(normalize_named_arg "ed-key-file" "$legacy_ed_dsa_key_file")"
    link="$(normalize_named_arg "link" "{{ link }}")"
    if [[ -n "$ed_key_file" && -n "$legacy_ed_dsa_key_file" && "$ed_key_file" != "$legacy_ed_dsa_key_file" ]]; then
        echo "Conflicting Sparkle key file arguments: ed-key-file and ed-dsa-key-file"
        exit 1
    fi
    sparkle_key_file="$ed_key_file"
    if [[ -z "$sparkle_key_file" ]]; then
        sparkle_key_file="$legacy_ed_dsa_key_file"
    fi
    SPARKLE_BIN="BoBeMacUI/.build/artifacts/sparkle/Sparkle/bin"
    if [[ ! -x "$SPARKLE_BIN/generate_appcast" ]]; then
        echo "Sparkle tools not found. Run: cd BoBeMacUI && swift package resolve"
        exit 1
    fi
    mkdir -p "$archives_dir"
    if [[ -n "$sparkle_key_file" && "$sparkle_key_file" != "-" && ! -f "$sparkle_key_file" ]]; then
        echo "Sparkle private key file not found: $sparkle_key_file"
        exit 1
    fi
    shopt -s nullglob
    archives=(
        "$archives_dir"/*.zip
        "$archives_dir"/*.tar
        "$archives_dir"/*.tar.gz
        "$archives_dir"/*.tgz
        "$archives_dir"/*.dmg
    )
    if (( ${#archives[@]} == 0 )); then
        echo "No staged Sparkle archives found in $archives_dir"
        echo "Stage a ZIP, DMG, or tarball there before running generate_appcast"
        exit 1
    fi
    ARGS=()
    if [[ -n "$download_url_prefix" ]]; then
        if [[ "$download_url_prefix" != */ ]]; then
            download_url_prefix="$download_url_prefix/"
        fi
        ARGS+=(--download-url-prefix "$download_url_prefix")
    fi
    if [[ -n "$link" ]]; then
        ARGS+=(--link "$link")
    fi
    if [[ -n "$sparkle_key_file" ]]; then
        ARGS+=(--ed-key-file "$sparkle_key_file")
    fi
    "$SPARKLE_BIN/generate_appcast" "${ARGS[@]}" "$archives_dir"
    echo "Generated appcast at $archives_dir/appcast.xml"

# Build debug + launch app (Swift app manages backend lifecycle)
run:
    #!/bin/bash
    set -euo pipefail
    COPILOT_CLI_DIR="{{ copilot_cli_dir }}"
    just build-backend debug
    (cd BoBeMacUI && swift build -c debug)
    # Place backend where BackendService.findBinaryPath() discovers it
    mkdir -p BoBeMacUI/.build/debug
    cp BoBeService/target/debug/bobe BoBeMacUI/.build/debug/bobe-daemon
    echo "Launching BoBe..."
    COPILOT_CLI_PATH="$COPILOT_CLI_DIR/copilot" BoBeMacUI/.build/debug/BoBe

# Run backend only (use when running frontend from Xcode)
backend:
    #!/bin/bash
    set -euo pipefail
    COPILOT_CLI_DIR="{{ copilot_cli_dir }}"
    just build-backend debug
    cd BoBeService
    exec env COPILOT_CLI_EXTRACT_DIR="$COPILOT_CLI_DIR" \
        BUNDLED_CLI_CACHE_DIR="{{ copilot_cli_cache_dir }}" \
        COPILOT_CLI_PATH="$COPILOT_CLI_DIR/copilot" \
        cargo run --locked --no-default-features -- serve

# Alias for backend
run-backend: backend

# Clean all build artifacts
clean-rust:
    cd BoBeService && cargo clean

clean: clean-rust
    cd BoBeMacUI && swift package clean
    rm -rf BoBeMacUI/.build/xcode-derived BoBeMacUI/BoBe.xcodeproj
    rm -rf build/

# Generate Xcode project (for previews/debugging)
xcode: check-tools
    cd BoBeMacUI && ../.tools/bin/xcodegen generate
    mkdir -p BoBeMacUI/BoBe.xcodeproj/project.xcworkspace/xcshareddata/swiftpm
    cp BoBeMacUI/Package.resolved BoBeMacUI/BoBe.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved

# Install pinned developer CLIs under .tools/ (never globally)
bootstrap-tools:
    ./scripts/bootstrap-dev-tools.sh

# Verify the project-local developer toolchain is present and pinned
check-tools:
    ./scripts/bootstrap-dev-tools.sh --check

# fmt + clippy + Rust/Swift tests + audits + builds
check: check-tools
    cd BoBeService && cargo fmt --check
    # Static checks never start Copilot; avoid embedding its CLI archive in
    # every clippy/test SDK artifact.
    cd BoBeService && COPILOT_SKIP_CLI_DOWNLOAD=1 CARGO_BUILD_WARNINGS=deny CARGO_INCREMENTAL=0 cargo clippy -q --locked --no-default-features
    cd BoBeService && COPILOT_SKIP_CLI_DOWNLOAD=1 CARGO_BUILD_WARNINGS=deny CARGO_INCREMENTAL=0 cargo test -q --locked --no-default-features
    cd BoBeService && ../.tools/bin/cargo-deny check
    cd BoBeService && ../.tools/bin/cargo-machete
    ./scripts/check-cross-language-constants.sh
    ./scripts/check-localization-catalogs.sh
    cd BoBeMacUI && ../.tools/bin/swiftlint lint --quiet
    cd BoBeMacUI && swift build -c debug
    ./scripts/check-swift-runtime-artifacts.sh debug
    cd BoBeMacUI && swift test

# Report daemon Cargo cache growth on demand without deleting active build state.
rust-target-size:
    #!/usr/bin/env bash
    set -euo pipefail
    target="BoBeService/target"
    if [[ ! -d "$target" ]]; then
        echo "Rust target: 0 GiB (not created)"
        exit 0
    fi
    if ! size_output="$(du -sk "$target" 2>/dev/null)"; then
        echo "warning: Rust target size changed during measurement; retry 'just rust-target-size'" >&2
        exit 0
    fi
    kib="$(awk 'END { print $1 }' <<<"$size_output")"
    if [[ -z "$kib" ]]; then
        echo "warning: Rust target size changed during measurement; retry 'just rust-target-size'" >&2
        exit 0
    fi
    gib="$(awk -v kib="$kib" 'BEGIN { printf "%.1f", kib / 1048576 }')"
    echo "Rust target: ${gib} GiB"
    for path in "$target/debug/deps" "$target/debug/incremental" "$target/debug/build" "$target/release"; do
        [[ -e "$path" ]] && { du -sh "$path" 2>/dev/null || true; }
    done
    if (( kib > 15 * 1024 * 1024 )); then
        echo "warning: Rust target exceeds 15 GiB; run 'just clean-rust' after a toolchain, profile, or dependency migration" >&2
    fi

# Alias for check (muscle memory)
test: check

# Full ship: clean → resolve deps → build → sign → DMG → notarize → staple → Sparkle zip
# Requires env var: NOTARIZE_KEYCHAIN_PROFILE
ship version identity="Developer ID Application":
    #!/bin/bash
    set -euo pipefail
    version="{{ version }}"
    tag="v$version"
    verify_release_state() {
        if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
            echo "Refusing to ship from a dirty worktree" >&2
            exit 1
        fi
        if ! git show-ref --verify --quiet "refs/tags/$tag"; then
            echo "Refusing to ship without local tag $tag" >&2
            exit 1
        fi
        if [[ "$(git rev-parse HEAD)" != "$(git rev-parse "$tag^{commit}")" ]]; then
            echo "Refusing to ship: HEAD does not match $tag" >&2
            exit 1
        fi
    }
    verify_release_state

    echo "=== Clean ==="
    just clean

    echo "=== Resolve dependencies ==="
    (cd BoBeService && cargo fetch --locked)
    (cd BoBeMacUI && swift package resolve)
    verify_release_state

    echo "=== Build + Bundle + Sign + DMG ==="
    just release "{{ version }}" "{{ identity }}"

    echo "=== Notarize ==="
    just notarize {{ version }}

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
