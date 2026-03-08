# Code Signing & Notarization

## Signing

Sign inside-out — embedded binaries first, then the app bundle.

```bash
# 1. Sign the Rust backend
codesign -s "Developer ID Application: YOUR_TEAM_NAME (TEAM_ID)" \
  --options runtime --timestamp \
  --entitlements BoBeMacUI/entitlements.plist \
  BoBe.app/Contents/MacOS/bobe-daemon

# 2. Sign the app bundle
codesign -s "Developer ID Application: YOUR_TEAM_NAME (TEAM_ID)" \
  --options runtime --timestamp \
  --entitlements BoBeMacUI/entitlements.plist \
  --force \
  BoBe.app
```

`--options runtime` enables Hardened Runtime (required for notarization). `--timestamp` embeds a secure timestamp.

## Entitlements

File: `BoBeMacUI/entitlements.plist`

| Entitlement | Value | Reason |
| ------------- | ------- | -------- |
| `com.apple.security.automation.apple-events` | `true` | Required for osascript System Events automation (active window detection) under hardened runtime |

## Notarization

```bash
# Submit and wait
xcrun notarytool submit BoBe.dmg \
  --apple-id "your@apple.id" --team-id "TEAM_ID" \
  --password "app-specific-password" --wait

# Staple the ticket
xcrun stapler staple BoBe.dmg
```

## Verification

```bash
codesign --verify --deep --strict --verbose=2 BoBe.app   # valid on disk
spctl --assess --type exec --verbose BoBe.app             # accepted
xcrun stapler validate BoBe.dmg                           # staple check
```

## Credentials (CI/CD)

| Secret | Env Var | Description |
| -------- | --------- | ------------- |
| Apple ID | `APPLE_ID` | Developer account email |
| Team ID | `APPLE_TEAM_ID` | 10-char team identifier |
| App password | `APPLE_APP_PASSWORD` | Generated at appleid.apple.com |
| Signing identity | `SIGNING_IDENTITY` | "Developer ID Application: ..." |

For local dev, use a keychain profile:

```bash
xcrun notarytool store-credentials "BoBe-notarize" \
  --apple-id "your@apple.id" --team-id "TEAM_ID" --password "app-specific-password"

# Then:
xcrun notarytool submit BoBe.dmg --keychain-profile "BoBe-notarize" --wait
```
