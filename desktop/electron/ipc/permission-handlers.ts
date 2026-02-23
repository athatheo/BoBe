/**
 * Permission IPC handlers
 *
 * Handles macOS permission checks (screen recording, data directory)
 * so the renderer can guard feature toggles and show actionable messages.
 *
 * macOS specifics:
 * - Screen recording: cannot be requested programmatically, must be granted in System Settings.
 *   Requires app restart to take effect after granting.
 * - Data directory (~/.bobe/): normal filesystem access, no TCC permission needed.
 */

import { app, ipcMain, shell, systemPreferences } from 'electron'
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { IPC_CHANNELS } from '../types'

// =============================================================================
// PERMISSION IPC HANDLERS
// =============================================================================

export function setupPermissionIpcHandlers(): void {
  ipcMain.handle(IPC_CHANNELS.PERMISSIONS_CHECK_SCREEN, () => {
    if (process.platform !== 'darwin') return 'granted'
    return systemPreferences.getMediaAccessStatus('screen')
  })

  ipcMain.handle(IPC_CHANNELS.PERMISSIONS_OPEN_SCREEN_SETTINGS, async () => {
    if (process.platform !== 'darwin') return
    await shell.openExternal(
      'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture',
    )
  })

  ipcMain.handle(IPC_CHANNELS.PERMISSIONS_CHECK_DATA_DIR, () => {
    return checkDataDirectory()
  })
}

// =============================================================================
// DATA DIRECTORY CHECK
// =============================================================================

/**
 * Check if ~/.bobe/ exists or can be created and written to.
 * Returns { ok: true } on success, { ok: false, error: string } on failure.
 */
export function checkDataDirectory(): { ok: boolean; error?: string } {
  const bobeDir = path.join(app.getPath('home'), '.bobe')
  try {
    mkdirSync(bobeDir, { recursive: true })
    const testFile = path.join(bobeDir, '.write-test')
    writeFileSync(testFile, '')
    rmSync(testFile)
    return { ok: true }
  } catch (err) {
    return { ok: false, error: String(err) }
  }
}
