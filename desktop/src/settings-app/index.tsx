/**
 * Settings app entry point
 *
 * React entry for the Bobe Tuning settings window.
 */

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { initializeTheme } from '@/hooks'
import '@/styles/globals.css'

// Monaco loader is deferred — initialized lazily when a Monaco-using
// component mounts (SoulsSettings, GoalsSettings, etc.).
// This keeps the settings window opening instant.

// Initialize theme before first render to prevent flash
initializeTheme()

const root = document.getElementById('root')
if (!root) {
  throw new Error('Root element not found')
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
