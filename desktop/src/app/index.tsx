/**
 * Application entry point
 */

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { initializeTheme } from '@/hooks'
import '@/styles/globals.css'

// Initialize theme before first render to prevent flash
initializeTheme()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
