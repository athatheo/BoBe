/**
 * Electron security hardening
 *
 * Implements the Electron security checklist:
 * - Custom app:// protocol (replaces file:// in production)
 * - Content-Security-Policy via session headers
 * - Navigation guards (blocks navigation + popup windows)
 * - Permission request denial
 * - Certificate error handling
 *
 * Must be initialized in two phases:
 * 1. registerAppProtocol() - BEFORE app.whenReady()
 * 2. initSecurity()        - INSIDE app.whenReady()
 *
 * @see https://www.electronjs.org/docs/latest/tutorial/security
 */

import { app, protocol, session, net } from 'electron'
import { isAbsolute, join, relative, resolve } from 'path'
import { pathToFileURL } from 'url'

/**
 * Uses app.isPackaged instead of process.env.NODE_ENV.
 * NODE_ENV can be tampered at runtime; app.isPackaged is set by Electron
 * at build time based on whether the app runs from an asar archive.
 */
const isDev = !app.isPackaged

// The renderer build output directory
const RENDERER_DIR = join(__dirname, '../renderer')

// Custom protocol scheme
const PROTOCOL_SCHEME = 'app'

// Daemon connection target (locked to specific address + port)
const DAEMON_ORIGIN = 'http://127.0.0.1:8766'

// Allowed dev server origins (only used when !app.isPackaged)
const DEV_ORIGINS = ['http://localhost:5173', 'http://127.0.0.1:5173']

// ---------------------------------------------------------------------------
// Content-Security-Policy
// ---------------------------------------------------------------------------

function getProductionCSP(): string {
  return [
    "default-src 'none'",
    `script-src ${PROTOCOL_SCHEME}://bobe`,
    // unsafe-inline needed for framer-motion + runtime CSS injection.
    // Acceptable: renderer only loads from app://bobe (no external content).
    `style-src ${PROTOCOL_SCHEME}://bobe 'unsafe-inline'`,
    `worker-src ${PROTOCOL_SCHEME}://bobe blob:`,
    `connect-src ${DAEMON_ORIGIN}`,
    `img-src ${PROTOCOL_SCHEME}://bobe data:`,
    `font-src ${PROTOCOL_SCHEME}://bobe`,
    `base-uri ${PROTOCOL_SCHEME}://bobe`,
    "form-action 'none'",
    "frame-ancestors 'none'",
    "object-src 'none'",
  ].join('; ')
}

function getDevelopmentCSP(): string {
  return [
    "default-src 'none'",
    "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
    "style-src 'self' 'unsafe-inline'",
    "worker-src 'self' blob:",
    `connect-src 'self' ws://localhost:* http://localhost:* http://127.0.0.1:*`,
    "img-src 'self' data:",
    "font-src 'self' data:",
    "base-uri 'self'",
    "form-action 'none'",
    "frame-ancestors 'none'",
    "object-src 'none'",
  ].join('; ')
}

// ---------------------------------------------------------------------------
// Phase 1: Register custom protocol scheme (BEFORE app ready)
// ---------------------------------------------------------------------------

/**
 * Register the app:// protocol scheme with appropriate privileges.
 * MUST be called before app 'ready' event fires.
 */
export function registerAppProtocol(): void {
  protocol.registerSchemesAsPrivileged([
    {
      scheme: PROTOCOL_SCHEME,
      privileges: {
        standard: true,
        secure: true,
        supportFetchAPI: true,
        stream: true,
        // bypassCSP is intentionally NOT set (defaults to false)
      },
    },
  ])
}

// ---------------------------------------------------------------------------
// Phase 2: Initialize all security measures (INSIDE app.whenReady)
// ---------------------------------------------------------------------------

/**
 * Initialize all security measures. Call inside app.whenReady().
 *
 * Sets up:
 * - Custom protocol handler with directory traversal protection
 * - CSP headers on all responses
 * - Navigation guards on all web contents
 * - Permission request handler (deny all by default)
 * - Certificate error handler
 */
export function initSecurity(): void {
  // --- Custom protocol handler (production only) ---
  if (!isDev) {
    protocol.handle(PROTOCOL_SCHEME, (request) => {
      const url = new URL(request.url)
      let filePath = decodeURIComponent(url.pathname)
      if (filePath === '/' || filePath === '') {
        filePath = '/index.html'
      }

      const resolvedPath = resolve(RENDERER_DIR, filePath.slice(1))

      // SECURITY: prevent directory traversal — resolved path must stay inside RENDERER_DIR
      // Uses path.relative() instead of startsWith() for robustness on case-insensitive FS
      const rel = relative(RENDERER_DIR, resolvedPath)
      if (rel.startsWith('..') || isAbsolute(rel)) {
        console.warn('[Security] Directory traversal blocked:', request.url)
        return new Response('Forbidden', { status: 403 })
      }

      return net.fetch(pathToFileURL(resolvedPath).toString())
    })
  }

  // --- CSP headers on all responses ---
  const csp = isDev ? getDevelopmentCSP() : getProductionCSP()
  session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
    callback({
      responseHeaders: {
        ...details.responseHeaders,
        'Content-Security-Policy': [csp],
      },
    })
  })

  // --- Navigation guards ---
  app.on('web-contents-created', (_event, contents) => {
    // Block all navigations (except dev HMR reloads)
    contents.on('will-navigate', (event, navigationUrl) => {
      if (isDev) {
        try {
          const parsed = new URL(navigationUrl)
          if (DEV_ORIGINS.includes(parsed.origin)) return
        } catch {
          // Invalid URL — fall through to block
        }
      }
      event.preventDefault()
      console.warn('[Security] Navigation blocked:', navigationUrl)
    })

    // Block all new window creation
    contents.setWindowOpenHandler(({ url }) => {
      console.warn('[Security] Window open blocked:', url)
      return { action: 'deny' }
    })
  })

  // --- Permission handlers: deny all requests ---
  session.defaultSession.setPermissionCheckHandler(
    (_webContents, _permission, _requestingOrigin, _details) => {
      return false
    },
  )

  session.defaultSession.setPermissionRequestHandler(
    (_webContents, _permission, callback, _details) => {
      callback(false)
    },
  )

  // --- Certificate error handler: reject invalid certs in production ---
  if (!isDev) {
    app.on('certificate-error', (event, _webContents, _url, _error, _certificate, callback) => {
      event.preventDefault()
      callback(false)
    })
  }
}

// ---------------------------------------------------------------------------
// Helpers for window loading
// ---------------------------------------------------------------------------

/**
 * Get the URL to load for a renderer page.
 * In dev: uses Vite dev server URL.
 * In production: uses app:// custom protocol.
 */
export function getRendererURL(page: string = 'index.html'): string {
  if (isDev && process.env['ELECTRON_RENDERER_URL']) {
    const base = process.env['ELECTRON_RENDERER_URL']
    return page === 'index.html' ? base : `${base}/${page}`
  }
  return `${PROTOCOL_SCHEME}://bobe/${page}`
}
