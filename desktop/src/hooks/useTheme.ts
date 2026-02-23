/**
 * useTheme hook
 *
 * Manages theme state and applies CSS variables for theming.
 * Uses localStorage for persistence.
 */

import { useSyncExternalStore, useCallback, useEffect } from 'react'
import type { ThemeId, ThemeConfig } from '@/types/settings'
import { THEMES, getThemeById } from '@/types/settings'

// =============================================================================
// CONSTANTS
// =============================================================================

const STORAGE_KEY = 'bobe-theme'
const DEFAULT_THEME: ThemeId = 'bauhaus'

// =============================================================================
// EXTERNAL STORE
// =============================================================================

interface ThemeState {
  themeId: ThemeId
  theme: ThemeConfig
}

let state: ThemeState = {
  themeId: DEFAULT_THEME,
  theme: getThemeById(DEFAULT_THEME),
}

const listeners = new Set<() => void>()

function subscribe(callback: () => void): () => void {
  listeners.add(callback)
  return () => listeners.delete(callback)
}

function getSnapshot(): ThemeState {
  return state
}

function setState(themeId: ThemeId): void {
  const theme = getThemeById(themeId)
  state = { themeId, theme }
  listeners.forEach((cb) => cb())
}

// =============================================================================
// THEME APPLICATION
// =============================================================================

/**
 * Apply theme colors to CSS variables
 */
function applyTheme(theme: ThemeConfig): void {
  const root = document.documentElement

  // Map theme colors to CSS variables
  root.style.setProperty('--color-bobe-terracotta', theme.colors.primary)
  root.style.setProperty('--color-bobe-olive', theme.colors.secondary)
  root.style.setProperty('--color-bobe-clay', theme.colors.tertiary)
  root.style.setProperty('--color-bobe-warm-white', theme.colors.background)
  root.style.setProperty('--color-bobe-sand', theme.colors.border)
  root.style.setProperty('--color-bobe-charcoal', theme.colors.text)

  // Avatar gradient colors
  root.style.setProperty('--color-avatar-face-light', theme.colors.avatarFaceLight)
  root.style.setProperty('--color-avatar-face-dark', theme.colors.avatarFaceDark)
  root.style.setProperty('--color-avatar-ring', theme.colors.avatarRing)
  root.style.setProperty('--color-avatar-iris', theme.colors.avatarIris)
  root.style.setProperty('--color-avatar-eye-outline', theme.colors.avatarEyeOutline)
  root.style.setProperty('--color-avatar-mouth', theme.colors.avatarMouth)

  // Add theme class and dark mode attribute for theme-specific CSS
  root.setAttribute('data-theme', theme.id)
  root.setAttribute('data-dark', theme.isDark ? 'true' : 'false')
}

/**
 * Load theme from localStorage
 */
function loadSavedTheme(): ThemeId {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved && THEMES.some((t) => t.id === saved)) {
      return saved as ThemeId
    }
  } catch {
    // localStorage might not be available
  }
  return DEFAULT_THEME
}

/**
 * Save theme to localStorage
 */
function saveTheme(themeId: ThemeId): void {
  try {
    localStorage.setItem(STORAGE_KEY, themeId)
  } catch {
    // localStorage might not be available
  }
}

// =============================================================================
// INITIALIZATION
// =============================================================================

// Load saved theme on module init
const savedTheme = loadSavedTheme()
setState(savedTheme)

// =============================================================================
// CROSS-WINDOW SYNC
// =============================================================================

const BROADCAST_CHANNEL_NAME = 'bobe-theme-sync'
let broadcastChannel: BroadcastChannel | null = null

/**
 * Set up cross-window theme synchronization using BroadcastChannel
 */
function setupCrossWindowSync(): void {
  if (typeof window === 'undefined') return

  try {
    broadcastChannel = new BroadcastChannel(BROADCAST_CHANNEL_NAME)
    broadcastChannel.onmessage = (event) => {
      const newThemeId = event.data as ThemeId
      if (THEMES.some((t) => t.id === newThemeId) && newThemeId !== state.themeId) {
        setState(newThemeId)
        applyTheme(getThemeById(newThemeId))
      }
    }
  } catch {
    // BroadcastChannel not supported, fall back to storage events
    window.addEventListener('storage', (event) => {
      if (event.key === STORAGE_KEY && event.newValue) {
        const newThemeId = event.newValue as ThemeId
        if (THEMES.some((t) => t.id === newThemeId) && newThemeId !== state.themeId) {
          setState(newThemeId)
          applyTheme(getThemeById(newThemeId))
        }
      }
    })
  }
}

/**
 * Broadcast theme change to other windows
 */
function broadcastThemeChange(themeId: ThemeId): void {
  broadcastChannel?.postMessage(themeId)
}

// Set up cross-window sync on module init
setupCrossWindowSync()

// =============================================================================
// HOOK
// =============================================================================

export function useTheme() {
  const currentState = useSyncExternalStore(subscribe, getSnapshot, getSnapshot)

  // Apply theme to DOM whenever it changes
  useEffect(() => {
    applyTheme(currentState.theme)
  }, [currentState.theme])

  // Change theme
  const setTheme = useCallback((themeId: ThemeId) => {
    setState(themeId)
    saveTheme(themeId)
    applyTheme(getThemeById(themeId))
    broadcastThemeChange(themeId)
  }, [])

  return {
    themeId: currentState.themeId,
    theme: currentState.theme,
    themes: THEMES,
    setTheme,
  }
}

/**
 * Initialize theme on app startup
 * Call this in the app root to apply theme before first render
 */
export function initializeTheme(): void {
  applyTheme(state.theme)
}
