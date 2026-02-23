/**
 * Manual fuse-flipping tool (for debugging / verification).
 *
 * In normal builds, fuses are set automatically via build/afterPack.js
 * during `pnpm pack` / `pnpm dist`. This script exists for manual use only.
 *
 * Usage: npx tsx scripts/set-fuses.ts <path-to-electron-binary>
 *
 * @see https://www.electronjs.org/docs/latest/tutorial/fuses
 */

import { flipFuses, FuseVersion, FuseV1Options } from '@electron/fuses'

const electronPath = process.argv[2]

if (!electronPath) {
  console.error('Usage: npx tsx scripts/set-fuses.ts <path-to-electron-binary>')
  console.error('  macOS: out/mac-arm64/BoBe.app')
  console.error('  Linux: out/linux-unpacked/bobe')
  process.exit(1)
}

async function main() {
  console.log(`Setting fuses on: ${electronPath}`)

  await flipFuses(electronPath, {
    version: FuseVersion.V1,

    // Disable ELECTRON_RUN_AS_NODE (prevents using app as generic Node.js)
    [FuseV1Options.RunAsNode]: false,

    // Encrypt cookie store with OS-level cryptography
    [FuseV1Options.EnableCookieEncryption]: true,

    // Disable NODE_OPTIONS env var (prevents injection of Node flags)
    [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false,

    // Disable --inspect / --inspect-brk debug flags
    [FuseV1Options.EnableNodeCliInspectArguments]: false,

    // Validate app.asar integrity on macOS/Windows
    [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true,

    // Only allow loading app from app.asar (prevents code tampering)
    [FuseV1Options.OnlyLoadAppFromAsar]: true,

    // Use separate V8 snapshots for main/renderer (security isolation)
    [FuseV1Options.LoadBrowserProcessSpecificV8Snapshot]: true,

    // Disable file:// protocol extra privileges (we use app:// instead)
    [FuseV1Options.GrantFileProtocolExtraPrivileges]: false,
  })

  console.log('Fuses set successfully!')
}

main().catch((err) => {
  console.error('Failed to set fuses:', err)
  process.exit(1)
})
