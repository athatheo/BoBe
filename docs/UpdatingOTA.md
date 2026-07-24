# Sparkle OTA and local release runbook

BoBe releases are built and published from a maintainer Mac. The repository has no GitHub Actions workflows; `just check` and the release recipes are the canonical automation.

## Artifact chain

1. Build the Rust daemon and Swift application.
2. Assemble `build/BoBe.app`.
3. Sign the Copilot helper, embedded daemon, and Sparkle helpers/framework inside-out, then the app bundle.
4. Build and sign `build/BoBe-X.Y.Z.dmg`.
5. Notarize, staple, and validate the app and DMG.
6. Create and sign `build/BoBe-X.Y.Z.zip` for Sparkle.
7. Generate an appcast from staged archives.
8. Publish the archive before the appcast that references it.

`SUPublicEDKey` and `SUFeedURL` live in `BoBeMacUI/BoBe/Resources/Info.plist`. The configured feed is:

```text
https://bobebot.com/updates/macos/appcast.xml
```

## Preflight

Use a clean, reviewed `main` checkout with no unrelated changes:

```bash
just check
git status --short
git tag -s vX.Y.Z -m "BoBe X.Y.Z"
```

`just ship` fails unless the worktree is clean and `HEAD` is exactly the commit
named by the local `vX.Y.Z` tag. Push that tag before creating the GitHub
Release.

Confirm that the Developer ID identity is available:

```bash
security find-identity -v -p codesigning
```

Release secrets and keys must remain outside the repository. Do not commit certificates, notarization credentials, Sparkle private keys, keychain exports, or deployment credentials.

## Release with Apple ID notarization

Store the app-specific password in the macOS Keychain once. `notarytool`
prompts for it without placing it in shell history or a process argument:

```bash
xcrun notarytool store-credentials "bobe-notary" \
  --apple-id "maintainer@example.com" \
  --team-id "TEAMID1234"
```

`just ship` runs clean, dependency resolution, build, signing, DMG creation, notarization, stapling, validation, and Sparkle ZIP creation:

```bash
export NOTARIZE_KEYCHAIN_PROFILE="bobe-notary"
just ship X.Y.Z "Developer ID Application: NAME (TEAMID1234)"
```

The profile name is not secret. Never pass the app-specific password to
`notarytool --password`; command arguments are visible to other local
processes.

## Release with an App Store Connect API key

Run the stages explicitly:

```bash
just clean
just release X.Y.Z "Developer ID Application: NAME (TEAMID1234)"
just notarize-api-key X.Y.Z /path/to/AuthKey.p8 KEY_ID ISSUER_ID
just staple X.Y.Z
just sparkle-zip X.Y.Z
```

## Sign the Sparkle archive

Sparkle's `sign_update` tool is resolved through SwiftPM:

```bash
just sparkle-sign-update X.Y.Z /path/to/sparkle-private-key
```

Save the emitted enclosure attributes for the generated appcast.

## Generate the appcast

Stage only archives intended for publication:

```bash
rm -rf build/sparkle
mkdir -p build/sparkle
cp "build/BoBe-X.Y.Z.zip" build/sparkle/

just sparkle-generate-appcast \
  build/sparkle \
  https://bobebot.com/updates/macos/ \
  /path/to/sparkle-private-key \
  https://www.bobebot.com
```

Inspect `build/sparkle/appcast.xml` before publication. Confirm the version, download URL, file length, and EdDSA signature.

## Verify the exact payload

```bash
test -x build/BoBe.app/Contents/Helpers/copilot
codesign --verify --strict --verbose=2 \
  build/BoBe.app/Contents/Helpers/copilot
codesign --verify --deep --strict --verbose=2 build/BoBe.app
spctl --assess --type execute --verbose=4 build/BoBe.app
xcrun stapler validate build/BoBe.app
xcrun stapler validate "build/BoBe-X.Y.Z.dmg"

VERIFY_DIR="$(mktemp -d "${TMPDIR:-/tmp}/bobe-sparkle-verify.XXXXXX")"
trap 'rm -rf "$VERIFY_DIR"' EXIT
ditto -x -k "build/BoBe-X.Y.Z.zip" "$VERIFY_DIR"
test -x "$VERIFY_DIR/BoBe.app/Contents/Helpers/copilot"
codesign --verify --strict --verbose=2 \
  "$VERIFY_DIR/BoBe.app/Contents/Helpers/copilot"
codesign --verify --deep --strict --verbose=2 "$VERIFY_DIR/BoBe.app"
xcrun stapler validate "$VERIFY_DIR/BoBe.app"
```

## Publish safely

Publish the notarized DMG to the GitHub Release that the README sends users to.
For a new, already-pushed version tag:

```bash
gh release create "vX.Y.Z" "build/BoBe-X.Y.Z.dmg" \
  --verify-tag \
  --title "BoBe X.Y.Z" \
  --generate-notes
```

If the release already exists, use `gh release upload "vX.Y.Z"
"build/BoBe-X.Y.Z.dmg" --clobber` instead. Confirm the DMG is downloadable
from the public Releases page.

Upload the Sparkle ZIP next. Only then replace the public appcast:

```bash
rsync -av "build/BoBe-X.Y.Z.zip" "$HOST:$PATH/"
rsync -av build/sparkle/appcast.xml "$HOST:$PATH/appcast.xml.next"
ssh "$HOST" "mv '$PATH/appcast.xml.next' '$PATH/appcast.xml'"
```

Verify with bounded requests:

```bash
curl --fail --location --connect-timeout 5 --max-time 20 \
  https://bobebot.com/updates/macos/appcast.xml
curl --fail --location --head --connect-timeout 5 --max-time 20 \
  "https://bobebot.com/updates/macos/BoBe-X.Y.Z.zip"
gh release view "vX.Y.Z" --json assets \
  --jq '.assets[] | select(.name == "BoBe-X.Y.Z.dmg") | .url'
```

If signing, notarization, staple validation, ZIP verification, or upload fails, stop. Keep the previous appcast live until the complete replacement archive is reachable.

## Troubleshooting

- **Check for Updates is disabled:** verify `SUFeedURL` is nonempty HTTPS.
- **Update rejected:** sign the exact ZIP being published and copy its emitted attributes into the appcast.
- **No update appears:** confirm the public appcast version is newer than the installed version and its archive URL is reachable.
- **Notarization fails:** inspect the `notarytool` log before retrying; do not staple an unaccepted artifact.
