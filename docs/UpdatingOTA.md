# Sparkle OTA Update Process (BoBe macOS)

This document explains the full OTA (over-the-air) update flow for the macOS app using Sparkle.

## What is already configured

- Sparkle keypair generated on this Mac using `generate_keys` (private key stored in Keychain).
- `SUPublicEDKey` set in:
  - `desktopMac/BoBe/Resources/Info.plist`
  - `desktopMac/Info.plist`
- `SUFeedURL` set to:
  - `https://bobebot.com/updates/macos/appcast.xml`

## What these keys mean

- `SUPublicEDKey` (public): shipped in the app and committed to git.
- Private Sparkle signing key (secret): stays on your machine/CI secret store; never commit it.
- `SUFeedURL`: URL where the app downloads `appcast.xml` to discover updates.

## One-time setup checklist

1. Keep the Sparkle public key in both plist files (already done).
2. Host an HTTPS updates path, for example:
   - `https://bobebot.com/updates/macos/appcast.xml`
   - `https://bobebot.com/updates/macos/BoBe-<version>.zip`
3. Ensure Sparkle tools exist:

   ```bash
   cd desktopMac
   swift package resolve
   ```

## Release process (every version)

Use version `X.Y.Z` below.

1. Build/sign/notarize app as usual.

   ```bash
   just build version=X.Y.Z
   just sign identity="Developer ID Application"
   # your notarize + staple flow
   ```

2. Create Sparkle update archive (zip of `.app`):

   ```bash
   just sparkle-zip version=X.Y.Z
   ```

3. Sign update archive with Sparkle private key:

   ```bash
   just sparkle-sign-update version=X.Y.Z
   ```

4. Stage archives for appcast generation:

   ```bash
   mkdir -p build/sparkle
   cp "build/BoBe-X.Y.Z.zip" build/sparkle/
   ```

5. Generate appcast XML:

   ```bash
   just sparkle-generate-appcast \
     archives_dir=build/sparkle \
     download_url_prefix=https://bobebot.com/updates/macos
   ```

   This creates `build/sparkle/appcast.xml`.

6. Publish to `bobebot.com` (upload ZIP first, appcast last).

   Why this order: Sparkle clients poll `appcast.xml`; if appcast is uploaded before the ZIP is reachable, users can see a broken update.

   Example (SSH host serving `https://bobebot.com/updates/macos/`):

   ```bash
   VERSION=X.Y.Z
   UPDATES_HOST="deploy@bobebot.com"
   UPDATES_PATH="/var/www/bobebot.com/updates/macos"

   # Ensure destination exists
   ssh "$UPDATES_HOST" "mkdir -p '$UPDATES_PATH'"

   # 1) Upload versioned archive first
   rsync -av --progress "build/sparkle/BoBe-$VERSION.zip" "$UPDATES_HOST:$UPDATES_PATH/"

   # 2) Upload appcast last (atomic replace)
   rsync -av --progress "build/sparkle/appcast.xml" "$UPDATES_HOST:$UPDATES_PATH/appcast.xml.next"
   ssh "$UPDATES_HOST" "mv '$UPDATES_PATH/appcast.xml.next' '$UPDATES_PATH/appcast.xml'"
   ```

   Resulting public URLs must be:
   - `https://bobebot.com/updates/macos/appcast.xml`
   - `https://bobebot.com/updates/macos/BoBe-X.Y.Z.zip`

7. Verify website artifacts immediately after upload:

   ```bash
   curl -fL "https://bobebot.com/updates/macos/appcast.xml" | head -n 40
   curl -fI "https://bobebot.com/updates/macos/BoBe-X.Y.Z.zip"
   ```

   Check that:
   - appcast includes `BoBe X.Y.Z` item
   - `<enclosure url="...BoBe-X.Y.Z.zip" ...>` matches uploaded filename
   - HTTP status is `200` for both files

8. Verify in app:
   - Tray menu -> **Check for Updates...**
   - or Settings -> **Check for Updates...**

## Git/CI policy

- Commit:
  - `SUFeedURL`
  - `SUPublicEDKey`
  - release scripts/justfile changes
- Never commit:
  - Sparkle private key
  - keychain exports unless encrypted in CI secrets

## Troubleshooting

- If "Check for Updates..." is disabled:
  - verify `SUFeedURL` is non-empty and valid HTTPS.
- If update is found but rejected:
  - ensure `sign_update` was run for that exact zip.
  - ensure appcast entry signature matches the uploaded file.
- If no update appears:
  - confirm appcast and zip are publicly reachable at the URLs in feed.
  - confirm app version in appcast is greater than installed version.
