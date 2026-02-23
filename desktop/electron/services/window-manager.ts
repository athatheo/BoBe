/**
 * Window manager service
 *
 * Handles overlay window creation and management.
 * Security (CSP, navigation, protocol) is handled by security.ts.
 */

import { app, BrowserWindow, screen } from 'electron'
import { join } from 'path'
import { WINDOW_SIZES } from '../../src/lib/constants'
import { getRendererURL } from './security'

const isDev = !app.isPackaged

/**
 * Electron-specific window configuration.
 * Shared constants come from WINDOW_SIZES, electron-only values defined here.
 */
const OVERLAY_CONFIG = {
  ...WINDOW_SIZES,
  HEIGHT_MIN_EXPANDED: 280, // Avatar + input only (no messages) - electron only
}

let overlayWindow: BrowserWindow | null = null
let overlayVisible = true

/**
 * Create the main overlay window
 */
export function createOverlayWindow(): BrowserWindow {
  const { width: screenWidth, height: screenHeight } = screen.getPrimaryDisplay().workAreaSize

  const window = new BrowserWindow({
    width: OVERLAY_CONFIG.WIDTH_COLLAPSED,
    height: OVERLAY_CONFIG.HEIGHT_COLLAPSED,
    x: screenWidth - OVERLAY_CONFIG.WIDTH_COLLAPSED - OVERLAY_CONFIG.MARGIN,
    y: screenHeight - OVERLAY_CONFIG.HEIGHT_COLLAPSED - OVERLAY_CONFIG.MARGIN,

    // Overlay behavior
    frame: false,
    transparent: true,
    alwaysOnTop: true,
    skipTaskbar: true,
    resizable: false,
    movable: true,
    hasShadow: false,

    // Security settings (explicit — don't rely on defaults)
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: !isDev,
      webSecurity: true,
      allowRunningInsecureContent: false,
    },
  })

  // Stay above all windows including fullscreen
  window.setAlwaysOnTop(true, 'screen-saver')
  window.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true })

  // Load the app
  window.loadURL(getRendererURL())
  if (isDev) {
    window.webContents.openDevTools({ mode: 'detach' })
  }

  overlayWindow = window
  return window
}

/**
 * Get the current overlay window
 */
export function getOverlayWindow(): BrowserWindow | null {
  return overlayWindow
}

/**
 * Toggle overlay visibility
 */
export function toggleOverlayVisibility(): boolean {
  overlayVisible = !overlayVisible
  overlayWindow?.[overlayVisible ? 'show' : 'hide']()
  return overlayVisible
}

/**
 * Check if overlay is currently visible
 */
export function isOverlayVisible(): boolean {
  return overlayVisible
}

/**
 * Resize window for speech bubble display (legacy boolean API).
 * Expands both width and height when chat is shown.
 * Window stays anchored to bottom-right corner.
 */
export function resizeForBubble(show: boolean): void {
  const newWidth = show ? OVERLAY_CONFIG.WIDTH_EXPANDED : OVERLAY_CONFIG.WIDTH_COLLAPSED
  const newHeight = show ? OVERLAY_CONFIG.HEIGHT_MIN_EXPANDED : OVERLAY_CONFIG.HEIGHT_COLLAPSED
  resizeWindow(newWidth, newHeight)
}

/**
 * Resize window to specific dimensions.
 * Window stays anchored to bottom-right corner.
 *
 * @param width - Target width in pixels
 * @param height - Target height in pixels
 */
export function resizeWindow(width: number, height: number): void {
  if (!overlayWindow) return

  const { width: screenWidth, height: screenHeight } = screen.getPrimaryDisplay().workAreaSize
  const bounds = overlayWindow.getBounds()

  // Clamp dimensions to valid range
  const newWidth = Math.max(OVERLAY_CONFIG.WIDTH_COLLAPSED, Math.min(width, screenWidth - 16))
  const newHeight = Math.max(
    OVERLAY_CONFIG.HEIGHT_COLLAPSED,
    Math.min(height, OVERLAY_CONFIG.HEIGHT_MAX),
  )

  // Keep window anchored to bottom-right: adjust X and Y as size changes
  const rightEdge = bounds.x + bounds.width
  const newX = rightEdge - newWidth

  const bottomEdge = bounds.y + bounds.height
  const newY = bottomEdge - newHeight

  // Clamp to screen bounds
  const finalX = Math.max(8, Math.min(newX, screenWidth - newWidth - 8))
  const finalY = Math.max(8, Math.min(newY, screenHeight - newHeight - 8))

  overlayWindow.setBounds(
    {
      x: finalX,
      y: finalY,
      width: newWidth,
      height: newHeight,
    },
    true,
  )
}
