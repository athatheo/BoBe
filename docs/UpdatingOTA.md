# Sparkle OTA Update Process

## Configuration (already done)

- Sparkle keypair generated (private key in Keychain)
- `SUPublicEDKey` and `SUFeedURL` set in `desktopMac/BoBe/Resources/Info.plist`
- Feed URL: `https://bobebot.com/updates/macos/appcast.xml`

## Release Process

```bash
# 1. Build, sign, notarize as usual
just build version=X.Y.Z
just sign identity="Developer ID Application"

# 2. Create + sign Sparkle update archive
just sparkle-zip version=X.Y.Z
just sparkle-sign-update version=X.Y.Z

# 3. Generate appcast
mkdir -p build/sparkle
cp "build/BoBe-X.Y.Z.zip" build/sparkle/
just sparkle-generate-appcast \
  archives_dir=build/sparkle \
  download_url_prefix=https://bobebot.com/updates/macos

# 4. Upload ZIP first, then appcast (so clients don't see a broken update)
rsync -av "build/sparkle/BoBe-X.Y.Z.zip" "$HOST:$PATH/"
rsync -av "build/sparkle/appcast.xml" "$HOST:$PATH/appcast.xml.next"
ssh "$HOST" "mv '$PATH/appcast.xml.next' '$PATH/appcast.xml'"

# 5. Verify
curl -fL "https://bobebot.com/updates/macos/appcast.xml" | head -n 40
curl -fI "https://bobebot.com/updates/macos/BoBe-X.Y.Z.zip"
```

## Git/CI Policy

- **Commit**: `SUFeedURL`, `SUPublicEDKey`, release scripts
- **Never commit**: Sparkle private key, keychain exports (use CI secrets)

## Troubleshooting

- **"Check for Updates..." disabled**: Verify `SUFeedURL` is non-empty valid HTTPS
- **Update rejected**: Ensure `sign_update` was run for that exact zip
- **No update appears**: Confirm both files are publicly reachable and appcast version > installed version
