/**
 * System tray manager service
 *
 * Handles tray icon and context menu.
 * Menu uses plain language. Debug submenu only in dev builds.
 */

import { Tray, Menu, nativeImage, app } from 'electron'
import type { BobeState } from '../types'
import { toggleOverlayVisibility, isOverlayVisible } from './window-manager'

const isDev = !app.isPackaged

// 16x16 filled circle, used as macOS template image
const TRAY_ICON_16 =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAMklEQVR4nGNgGMzgPxomWyNJBhHSTNAQigwgVjNOQ4aBAaQYghNQbAAxhhANyNZIXwAABnZvkUpxlIEAAAAASUVORK5CYII='
// 32x32 filled circle for @2x displays
const TRAY_ICON_32 =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAVklEQVR4nO3O0QkAIAxDwe6/tE4gtGlqQBPI97sIz8O2DpdEr2CqcSoCjVMQ3XgLwYpDCHa8jPgbMBVPIwwwQA6YRKRnwAQCmjTOQlAmjaOI0Umi3pvbKXS+UMhFB0sAAAAASUVORK5CYII='

let tray: Tray | null = null

interface TrayCallbacks {
  onToggleCapture: () => void
  onDebugAction: (action: string) => void
  onOpenSettings: () => void
}

let callbacks: TrayCallbacks | null = null

function createTrayIcon(): Electron.NativeImage {
  const icon16 = nativeImage.createFromDataURL(TRAY_ICON_16)
  // Add @2x representation for Retina displays
  icon16.addRepresentation({ scaleFactor: 2.0, dataURL: TRAY_ICON_32 })
  icon16.setTemplateImage(true)
  return icon16
}

/**
 * Human-readable status text from state.
 */
function getStatusText(state: BobeState): string {
  if (!state.daemonConnected) return 'BoBe: Starting up...'
  if (state.thinking) return 'BoBe: Thinking...'
  if (state.speaking) return 'BoBe: Talking to you'
  if (state.capturing) return 'BoBe: Watching your screen'
  return 'BoBe: Ready'
}

/**
 * Create the system tray icon
 */
export function createTray(cbs: TrayCallbacks): Tray | null {
  callbacks = cbs

  try {
    const icon = createTrayIcon()
    tray = new Tray(icon)
    tray.setToolTip('BoBe')
    return tray
  } catch (err) {
    console.error('Failed to create tray:', err)
    return null
  }
}

/**
 * Update tray menu based on current state
 */
export function updateTrayMenu(state: BobeState): void {
  if (!tray || !callbacks) return

  const template: Electron.MenuItemConstructorOptions[] = [
    // Status
    { label: getStatusText(state), enabled: false },
    { type: 'separator' },

    // Capture toggle — plain language
    {
      label: state.capturing ? 'Stop Looking' : 'Allow Looking',
      click: () => callbacks?.onToggleCapture(),
    },

    // Show/Hide
    {
      label: isOverlayVisible() ? 'Hide BoBe' : 'Show BoBe',
      click: () => {
        toggleOverlayVisibility()
        // Rebuild tray menu to reflect new visibility state
        updateTrayMenu(state)
      },
    },

    { type: 'separator' },

    // Settings
    {
      label: 'Tune BoBe',
      click: () => callbacks?.onOpenSettings(),
    },
  ]

  // Debug submenu — dev builds only
  if (isDev) {
    template.push(
      { type: 'separator' },
      {
        label: 'Debug',
        submenu: createDebugSubmenu(),
      },
    )
  }

  template.push(
    { type: 'separator' },
    {
      label: 'Quit BoBe',
      click: () => app.quit(),
    },
  )

  tray.setContextMenu(Menu.buildFromTemplate(template))
}

function createDebugSubmenu(): Electron.MenuItemConstructorOptions[] {
  if (!callbacks) return []

  return [
    { label: 'Simulate: Loading', click: () => callbacks?.onDebugAction('loading') },
    { label: 'Simulate: Idle', click: () => callbacks?.onDebugAction('idle') },
    { label: 'Simulate: Capturing', click: () => callbacks?.onDebugAction('capturing') },
    { label: 'Simulate: Thinking', click: () => callbacks?.onDebugAction('thinking') },
    { label: 'Simulate: Wants to Speak', click: () => callbacks?.onDebugAction('wants_to_speak') },
    { label: 'Simulate: Speaking', click: () => callbacks?.onDebugAction('speaking') },
    { type: 'separator' },
    { label: 'Set Test Message', click: () => callbacks?.onDebugAction('set_message') },
    { label: 'Clear Message', click: () => callbacks?.onDebugAction('clear_message') },
    { type: 'separator' },
    { label: 'Clear All', click: () => callbacks?.onDebugAction('clear_all') },
  ]
}

/**
 * Get tray instance
 */
export function getTray(): Tray | null {
  return tray
}
