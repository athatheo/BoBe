/**
 * IPC handlers for app data management (delete all data, get data size)
 */

import { app, dialog, ipcMain } from 'electron'
import { readdirSync, rmSync, statSync } from 'node:fs'
import path from 'node:path'
import { backendService } from '../services/rust-service'

export function setupAppDataIpcHandlers(): void {
  /**
   * Get total size of app data (models, DB, logs, cache, etc.)
   */
  ipcMain.handle('app:get-data-size', () => {
    const userData = app.getPath('userData')
    const breakdown: Record<string, number> = {}

    const dirs = ['data', 'models', 'logs', 'ollama']
    for (const dir of dirs) {
      const dirPath = path.join(userData, dir)
      try {
        breakdown[dir] = getDirSizeMB(dirPath)
      } catch {
        breakdown[dir] = 0
      }
    }

    const totalMB = Object.values(breakdown).reduce((sum, v) => sum + v, 0)
    return { totalMB: Math.round(totalMB * 10) / 10, breakdown }
  })

  /**
   * Delete all app data after confirmation dialog.
   * Shows a native OS warning dialog, then:
   * 1. Stops the Python service
   * 2. Deletes everything in userData except Electron internals
   * 3. Quits the app (next launch triggers fresh setup)
   */
  ipcMain.handle('app:delete-all-data', async () => {
    const { response } = await dialog.showMessageBox({
      type: 'warning',
      buttons: ['Delete Everything', 'Cancel'],
      defaultId: 1,
      cancelId: 1,
      title: 'Delete All BoBe Data',
      message: 'Are you sure you want to delete all BoBe data?',
      detail:
        'This will permanently delete:\n' +
        '\n' +
        '  - Your database (memories, conversations, goals)\n' +
        '  - Downloaded AI models\n' +
        '  - Ollama binary and models\n' +
        '  - Logs and configuration\n' +
        '\n' +
        "BoBe will quit and you'll need to set up again on next launch.\n" +
        'This cannot be undone.',
    })

    if (response !== 0) {
      return // User cancelled
    }

    console.log('[AppData] Deleting all app data...')

    // Stop the backend service first
    try {
      await backendService.stop()
    } catch {
      // Continue even if stop fails
    }

    // Delete our data directories
    const userData = app.getPath('userData')
    const toDelete = [
      'data',
      'models',
      'logs',
      'ollama',
      'config.json',
      'db.key',
      'bobe-service.pid',
    ]

    for (const name of toDelete) {
      const fullPath = path.join(userData, name)
      try {
        rmSync(fullPath, { recursive: true, force: true })
        console.log(`[AppData] Deleted: ${name}`)
      } catch {
        console.warn(`[AppData] Failed to delete: ${name}`)
      }
    }

    console.log('[AppData] All data deleted. Quitting...')
    app.quit()
  })
}

/** Recursively calculate directory size in MB (with depth limit and symlink protection) */
function getDirSizeMB(dirPath: string, depth = 0): number {
  if (depth > 10) return 0 // Prevent infinite recursion

  let totalBytes = 0
  try {
    const entries = readdirSync(dirPath, { withFileTypes: true })
    for (const entry of entries) {
      if (entry.isSymbolicLink()) continue // Skip symlinks to avoid loops
      const fullPath = path.join(dirPath, entry.name)
      if (entry.isDirectory()) {
        totalBytes += getDirSizeMB(fullPath, depth + 1) * 1024 * 1024
      } else {
        try {
          totalBytes += statSync(fullPath).size
        } catch {
          // Skip inaccessible files
        }
      }
    }
  } catch {
    // Directory doesn't exist
  }
  return totalBytes / (1024 * 1024)
}
