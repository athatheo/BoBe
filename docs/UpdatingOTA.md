# Sparkle OTA and release automation

## What this document is for

This is the maintainer-facing runbook for how BoBe release automation is supposed to work after the CI/release hardening pass.

BoBe is not "just a Rust release":

- Rust builds `bobe-daemon`,
- Swift builds `BoBe`,
- the release bundles both into `BoBe.app`,
- then signs, notarizes, packages, and prepares update metadata around that bundle.

## System overview

The intended artifact chain is:

1. build the Rust daemon in release mode,
2. build the Swift app in release mode,
3. assemble `BoBe.app` with both binaries,
4. sign the embedded daemon,
5. sign the app bundle,
6. build and sign the DMG,
7. notarize the DMG,
8. staple and validate both `BoBe.app` and the DMG,
9. create a Sparkle ZIP from the stapled app,
10. extract the ZIP and verify its app signature and staple,
11. sign the Sparkle ZIP,
12. generate an appcast from the staged ZIP,
13. optionally create a GitHub Release and a website deployment PR for the appcast.

## Trust boundaries

There are two different automation lanes and they must stay separate.

| Lane | Trigger | Secrets | Purpose |
| --- | --- | --- | --- |
| Public vetting CI | PRs and normal pushes | None | Deterministic validation and dependency/supply-chain checks |
| Protected release workflow | Manual dispatch / trusted release refs | Environment-scoped release secrets | Build, sign, notarize, and optionally create release/deployment PR artifacts |

## Configuration (already done)

- Sparkle keypair generated (private key in Keychain)
- `SUPublicEDKey` and `SUFeedURL` set in `BoBeMacUI/BoBe/Resources/Info.plist`
- Feed URL: `https://bobebot.com/updates/macos/appcast.xml`

## CI / release model

Use two separate automation lanes:

- **Public vetting CI**
  - no release secrets,
  - runs deterministic validation and supply-chain checks,
  - never signs or publishes artifacts.
- **Protected release workflow**
  - uses macOS for signing/notarization and Ubuntu for release/PR orchestration,
  - guarded by GitHub environment approval,
  - has access to signing, notarization, Sparkle, and website-PR credentials,
  - rebuilds from the trusted release ref instead of promoting untrusted PR artifacts.

## How public vetting CI should work

The public workflow lives in `.github/workflows/ci.yml` and is intentionally secret-free.

It should:

1. check out the repo,
2. install pinned validation tools,
3. run `just check-ci`.

`just check-ci` is the CI contract:

```bash
cargo fmt --check
cargo clippy --locked -q
cargo test --locked -q
cargo deny check
cargo machete
cd BoBeMacUI && swiftlint lint --quiet
cd BoBeMacUI && swift build -c debug
```

Important notes:

- `--locked` makes CI use the reviewed dependency graph.
- This workflow must never gain signing, notarization, Sparkle, or publish credentials.

## How protected release orchestration should work

The protected workflow lives in `.github/workflows/release.yml`.

It is split into two jobs so secrets are not all exposed in one place:

### 1. `build-sign-notarize`

Environment: `release-signing`

Responsibilities:

- rebuild from the trusted release ref,
- import the Developer ID certificate into a temporary keychain,
- materialize the App Store Connect API key and Sparkle private key only at runtime,
- run:
  - `just release`
  - `just notarize-api-key`
  - `just staple` (staples and validates the app and DMG)
  - `just sparkle-zip`
  - extract the ZIP and verify `codesign` and `stapler validate` on its app
  - `just sparkle-sign-update`
- upload the signed DMG and signed Sparkle ZIP as GitHub artifacts,
- delete the temporary keychain and key material at the end.

### 2. `create-appcast-deployment-pr`

Environment: `release-publish`

Responsibilities:

- download and checksum the already signed release artifacts,
- create or update the GitHub Release assets,
- copy the generated appcast into a branch in the website repository,
- create a pull request for review and deployment by the website's actual process.

Important notes:

- `create_appcast_deployment_pr=true` does **not** publish the appcast or prove that the public feed changed.
- Merge and deployment permissions/processes for the website are outside this repository and are not assumed by the workflow.
- Verify the public feed only after that PR has actually been merged and deployed. Any optional probe should use short network timeouts and must not wait indefinitely.
- Release jobs rebuild from the trusted release ref; they do not sign artifacts produced by PR jobs or fork contexts.

## Release Process

```bash
# 1. Build, sign, notarize, staple, and validate
just release X.Y.Z "Developer ID Application"
just notarize-api-key X.Y.Z /path/to/AuthKey.p8 KEY_ID ISSUER_ID
just staple X.Y.Z

# 2. Create + sign Sparkle update archive from the stapled app
just sparkle-zip X.Y.Z
just sparkle-sign-update X.Y.Z /path/to/sparkle-private-key

# 3. Generate appcast
mkdir -p build/sparkle
cp "build/BoBe-X.Y.Z.zip" build/sparkle/
just sparkle-generate-appcast \
  build/sparkle \
  https://bobebot.com/updates/macos \
  /path/to/sparkle-private-key

# 4. Upload ZIP first, then appcast (so clients don't see a broken update)
rsync -av "build/sparkle/BoBe-X.Y.Z.zip" "$HOST:$PATH/"
rsync -av "build/sparkle/appcast.xml" "$HOST:$PATH/appcast.xml.next"
ssh "$HOST" "mv '$PATH/appcast.xml.next' '$PATH/appcast.xml'"

# 5. Verify
curl -fL "https://bobebot.com/updates/macos/appcast.xml" | head -n 40
curl -fI "https://bobebot.com/updates/macos/BoBe-X.Y.Z.zip"
```

## Operator flow

Recommended maintainer flow:

1. Merge the release candidate changes and make sure public vetting CI is green.
2. Choose the trusted release ref/tag.
3. Run the `Release` workflow with:
   - `version=X.Y.Z`
   - `create_appcast_deployment_pr=false` for a sign/notarize-only run.
4. Approve the `release-signing` environment when GitHub prompts for it.
5. Inspect the signed DMG and ZIP artifacts.
6. When ready, rerun with `create_appcast_deployment_pr=true` (or select it initially) to create the GitHub Release and website appcast deployment PR.
7. Approve the `release-publish` environment when prompted.
8. Review and merge the website PR through that repository's deployment process.
9. Only after deployment, verify with bounded requests such as:
   - `curl --fail --location --connect-timeout 5 --max-time 20 https://bobebot.com/updates/macos/appcast.xml`
   - ZIP reachability at the GitHub Release URL,
   - local Sparkle update behavior.

## Failure behavior

The intended safety properties are:

- if signing or notarization fails, no release or deployment PR is created;
- if app/DMG staple validation or extracted-ZIP verification fails, packaging stops;
- with respect to the appcast, a successful workflow only creates its deployment PR; it does not establish that the website merged or deployed it;
- clients continue seeing the prior feed until the website's reviewed deployment completes.

## CI-friendly notarization

For CI, prefer an App Store Connect API key over Apple ID password flows:

```bash
just notarize-api-key \
  X.Y.Z \
  /path/to/AuthKey_ABC123DEFG.p8 \
  ABC123DEFG \
  11223344-5566-7788-9900-aabbccddeeff
```

## Recommended GitHub environments

Split release secrets so one workflow compromise does not expose everything at once:

### `release-signing`

- `APPLE_SIGNING_IDENTITY`
- `APPLE_CERTIFICATE_P12_B64`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_KEYCHAIN_PASSWORD`
- `APPLE_API_KEY_P8_B64`
- `APPLE_API_KEY_ID`
- `APPLE_API_ISSUER_ID`
- `SPARKLE_PRIVATE_KEY_B64`

### `release-publish`

- `WEBSITE_DEPLOY_TOKEN` (fine-grained PAT with permission to push a branch and open a pull request on `johnkozaris/BoBeWebsite`)

The website repository's merge and deployment authorization remain separate and are not evidenced by this token.

## Recommended GitHub repo settings

These protections live in GitHub, not in the repository contents:

- enable branch protection on the default branch,
- protect release tags or require manual dispatch from a trusted ref,
- require reviewer approval on the `release-signing` and `release-publish` environments,
- disable self-approval for release environments if available,
- require CODEOWNERS review for:
  - `.github/workflows/**`
  - `justfile`
  - `docs/UpdatingOTA.md`
  - `BoBeMacUI/entitlements.plist`
  - `BoBeMacUI/BoBe/Resources/Info.plist`

These settings are not stored in the repository and must be configured manually in GitHub.

## Git/CI Policy

- **Commit**: `SUFeedURL`, `SUPublicEDKey`, release scripts
- **Never commit**: Sparkle private key, code-signing certificates, notarization API keys, deploy keys, or keychain exports
- **Protect**: workflow files, release scripts, and OTA docs with the same care as source code
- **Deployment order**: publish release assets before merging/deploying an appcast that references them
- **Cleanup**: CI should materialize keys only at runtime and delete them after use

## What still needs manual GitHub setup

The repository now contains the workflows and docs, but the following still must be done in GitHub itself:

- create the `release-signing` and `release-publish` environments,
- add the environment secrets listed above,
- require reviewers on both environments,
- disable self-approval if your plan/repo settings allow it,
- configure protected branches and/or trusted release tag rules,
- add CODEOWNERS rules for the release control plane.

## Troubleshooting

- **"Check for Updates..." disabled**: Verify `SUFeedURL` is non-empty valid HTTPS
- **Update rejected**: Ensure `sign_update` was run for that exact zip
- **No update appears**: Confirm both files are publicly reachable and appcast version > installed version
