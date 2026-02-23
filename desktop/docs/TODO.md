# Technical TODO

## Low Priority

### Test Infrastructure

- Add Vitest for unit tests
- ~~Add Playwright for E2E tests~~ ✅ Done
- Create mock provider for testing components

### Packaging & Distribution (electron-builder)

- Add electron-builder for creating distributable `.app`/`.exe`/`.dmg`
- Configure auto-update mechanism

**⚠️ Security Notes for Open Source:**

1. **Signing keys must NEVER be in the repo** - Use CI secrets (GitHub Actions secrets)
2. **Fuse hardening happens post-packaging** - Create `scripts/flip-fuses.js` that runs in CI after electron-builder
3. **Separate build vs release pipelines:**
   - `npm run build` → Development build (anyone can run, unsigned)
   - CI release workflow → Signed production build (maintainers only, uses secrets)
4. **Required CI secrets for signing:**
   - macOS: `CSC_LINK`, `CSC_KEY_PASSWORD`, `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID`
   - Windows: `CSC_LINK`, `CSC_KEY_PASSWORD` (for EV code signing)
5. **Fuses to flip in production builds:**
   ```javascript
   // scripts/flip-fuses.js (runs in CI only)
   FuseV1Options.RunAsNode: false
   FuseV1Options.EnableNodeCliInspectArguments: false  // ⚠️ Breaks Playwright!
   FuseV1Options.EnableNodeOptionsEnvironmentVariable: false
   FuseV1Options.EnableCookieEncryption: true
   FuseV1Options.OnlyLoadAppFromAsar: true
   ```

### Performance Monitoring

- Add performance marks for IPC timing
- Monitor renderer memory usage
- Track animation frame rates

---

## Completed

- [x] Create bobe-store with useBobe + useBobeSelector hooks
- [x] Convert Avatar to Tailwind CSS
- [x] Create proper type definitions for IPC
- [x] Add channel allowlisting to preload
- [x] Add Playwright E2E testing infrastructure
