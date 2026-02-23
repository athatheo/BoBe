/**
 * Electron main process entry point
 *
 * Two modes:
 *   DEV MODE (pnpm dev):
 *     - No setup wizard, no process management
 *     - You run `bobe serve` manually in a separate terminal
 *     - Electron just connects to localhost:8766 via DaemonClient
 *
 *   PRODUCTION MODE (packaged .app):
 *     - Checks app is in /Applications (macOS translocation protection)
 *     - Starts BackendService (health-checks or spawns bobe serve)
 *     - Asks service /onboarding/status — if needs_onboarding, shows wizard
 *     - Wizard proxies to service /onboarding/* endpoints (service owns setup logic)
 *     - On quit: SIGTERM → wait 5s → SIGKILL, then exit
 */

import { app, BrowserWindow, dialog } from 'electron'
import { existsSync } from 'node:fs'
import path from 'node:path'
import { registerAppProtocol, initSecurity } from '../services/security'
import {
  createOverlayWindow,
  createTray,
  updateTrayMenu,
  getTray,
  daemonClient,
  backendService,
  checkOnboardingStatus,
  runSetupWizard,
  hasEncryptionKey,
  openSettingsWindow,
  closeSettingsWindow,
} from '../services'
import {
  setupIpcHandlers,
  setupSettingsIpcHandlers,
  setupAppDataIpcHandlers,
  initDaemonEventHandlers,
  getState,
  setState,
  debugActions,
  checkDataDirectory,
} from '../ipc'

// =============================================================================
// MODE DETECTION
// =============================================================================

const packaged = app.isPackaged

function hasBundledBinary(): boolean {
  return existsSync(path.join(process.resourcesPath, 'bin', 'bobe'))
}

function shouldManageService(): boolean {
  return packaged || hasBundledBinary()
}

// =============================================================================
// APP IDENTITY: Set name before anything else (fixes dock showing "Electron" in dev)
// =============================================================================
app.setName('BoBe')

// =============================================================================
// SECURITY: Register custom protocol BEFORE app ready
// =============================================================================
registerAppProtocol()

const gotTheLock = app.requestSingleInstanceLock()
if (!gotTheLock) {
  app.quit()
}

let isQuitting = false

// =============================================================================
// APP LIFECYCLE
// =============================================================================

app.whenReady().then(async () => {
  initSecurity()

  // Pre-flight: ensure data directory (~/.bobe/) is writable
  const dataDirResult = checkDataDirectory()
  if (!dataDirResult.ok) {
    dialog.showErrorBox(
      'BoBe cannot start',
      'BoBe cannot create its data directory (~/.bobe). Check your file permissions.\n\n' +
        (dataDirResult.error || ''),
    )
    app.quit()
    return
  }

  // Set dock icon in dev mode (packaged builds get it from Info.plist)
  if (!packaged && process.platform === 'darwin' && app.dock) {
    const iconPath = path.join(__dirname, '../../resources/icon.icns')
    if (existsSync(iconPath)) {
      app.dock.setIcon(iconPath)
    }
  }

  // macOS: Offer to move to /Applications (prevents translocation)
  if (packaged && process.platform === 'darwin') {
    try {
      if (!app.isInApplicationsFolder()) {
        const { response } = await dialog.showMessageBox({
          type: 'question',
          buttons: ['Move to Applications', 'Keep Current Location'],
          defaultId: 0,
          message: 'Move BoBe to Applications?',
          detail: 'For the best experience, BoBe should run from the Applications folder.',
        })
        if (response === 0) {
          app.moveToApplicationsFolder()
          return
        }
      }
    } catch {
      // Continue
    }
  }

  // Setup IPC handlers
  setupIpcHandlers()
  setupSettingsIpcHandlers()
  setupAppDataIpcHandlers()
  initDaemonEventHandlers()

  // --- PRODUCTION: start service, then check if onboarding needed ---
  if (shouldManageService()) {
    console.log('[Main] Starting backend service...')
    backendService.on('fatal', () => {
      console.error('[Main] Backend service fatal error')
      updateTrayMenu(getState())
    })

    try {
      await backendService.start()
      console.log('[Main] Backend service ready')
    } catch (error) {
      console.error('[Main] Failed to start backend service:', error)
    }

    // Service is up — ask it if onboarding is needed
    const status = await checkOnboardingStatus()
    if (status?.needs_onboarding || !hasEncryptionKey()) {
      console.log('[Main] Onboarding needed — launching wizard')
      await runSetupWizard('first-run')
      console.log('[Main] Setup wizard complete')
    } else if (status && !status.complete) {
      // Configured before but something is broken (e.g. model deleted)
      console.log('[Main] Service degraded — launching wizard')
      await runSetupWizard('missing-llm')
    }
  } else {
    console.log('[Main] Dev mode: skipping service management (run `bobe serve` manually)')
  }

  // Create overlay window
  createOverlayWindow()

  // Create tray
  createTray({
    onToggleCapture: () => {
      setState({ capturing: !getState().capturing })
      updateTrayMenu(getState())
    },
    onDebugAction: (action: string) => {
      const debugAction = debugActions[action as keyof typeof debugActions]
      if (debugAction) {
        debugAction()
        updateTrayMenu(getState())
      }
    },
    onOpenSettings: () => {
      openSettingsWindow()
    },
  })

  updateTrayMenu(getState())

  // Connect daemon client (SSE for real-time state)
  console.log('[Main] Connecting to daemon...')
  await daemonClient.connect()

  daemonClient.on('connected', () => {
    if (getTray()) updateTrayMenu(getState())
  })
  daemonClient.on('disconnected', () => {
    if (getTray()) updateTrayMenu(getState())
  })

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createOverlayWindow()
    }
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit()
  }
})

// Graceful shutdown
app.on('before-quit', (e) => {
  if (isQuitting) return
  isQuitting = true

  // Close settings window gracefully before shutdown
  closeSettingsWindow()

  if (!shouldManageService()) {
    daemonClient.disconnect()
    return
  }

  e.preventDefault()
  console.log('[Main] Shutting down...')
  daemonClient.disconnect()
  backendService.stop().finally(() => {
    console.log('[Main] Cleanup complete, exiting')
    app.exit(0)
  })
})
