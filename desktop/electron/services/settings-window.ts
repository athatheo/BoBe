/**
 * Settings window manager
 *
 * Creates and manages the settings window for Bobe Tuning.
 * Follows single-instance pattern - if already open, focuses existing window.
 * Security (CSP, navigation, protocol) is handled by security.ts.
 */

import { BrowserWindow, screen } from 'electron'
import { join } from 'path'
import { getRendererURL } from './security'

// Window configuration
const SETTINGS_CONFIG = {
  WIDTH: 1050,
  HEIGHT: 720,
  MIN_WIDTH: 800,
  MIN_HEIGHT: 550,
}

let settingsWindow: BrowserWindow | null = null

/**
 * Create or focus the settings window
 */
export function openSettingsWindow(): BrowserWindow {
  // If window exists and isn't destroyed, restore and focus it
  if (settingsWindow && !settingsWindow.isDestroyed()) {
    if (settingsWindow.isMinimized()) {
      settingsWindow.restore()
    }
    settingsWindow.focus()
    return settingsWindow
  }

  // Get screen dimensions to center the window
  const { width: screenWidth, height: screenHeight } = screen.getPrimaryDisplay().workAreaSize
  const x = Math.floor((screenWidth - SETTINGS_CONFIG.WIDTH) / 2)
  const y = Math.floor((screenHeight - SETTINGS_CONFIG.HEIGHT) / 2)

  settingsWindow = new BrowserWindow({
    width: SETTINGS_CONFIG.WIDTH,
    height: SETTINGS_CONFIG.HEIGHT,
    minWidth: SETTINGS_CONFIG.MIN_WIDTH,
    minHeight: SETTINGS_CONFIG.MIN_HEIGHT,
    x,
    y,

    // Standard window behavior (not an overlay)
    frame: true,
    transparent: false,
    alwaysOnTop: false,
    resizable: true,
    movable: true,

    // Window styling
    title: 'Bobe Tuning',
    backgroundColor: '#FAF7F2', // --color-bobe-warm-white
    titleBarStyle: 'hiddenInset', // macOS traffic lights with custom content
    trafficLightPosition: { x: 16, y: 16 },

    // Security settings (explicit — don't rely on defaults)
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
      allowRunningInsecureContent: false,
    },
  })

  // Load the settings app
  settingsWindow.loadURL(getRendererURL('settings.html'))

  // Clean up reference when closed
  settingsWindow.on('closed', () => {
    settingsWindow = null
  })

  return settingsWindow
}

/**
 * Close the settings window if open
 */
export function closeSettingsWindow(): void {
  if (settingsWindow && !settingsWindow.isDestroyed()) {
    settingsWindow.close()
    settingsWindow = null
  }
}
