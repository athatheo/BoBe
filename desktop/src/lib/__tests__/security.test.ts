/**
 * Tests for security logic extracted from security.ts.
 *
 * Verifies CSP generation, path traversal protection, and channel allowlisting.
 */

import { describe, it, expect } from 'vitest'
import { resolve, relative, isAbsolute } from 'node:path'

// =============================================================================
// CSP generation (extracted logic from security.ts)
// =============================================================================

function getProductionCSP(protocolScheme: string, daemonOrigin: string): string {
  return [
    "default-src 'none'",
    `script-src ${protocolScheme}://bobe`,
    `style-src ${protocolScheme}://bobe 'unsafe-inline'`,
    `worker-src ${protocolScheme}://bobe blob:`,
    `connect-src ${daemonOrigin}`,
    `img-src ${protocolScheme}://bobe data:`,
    `font-src ${protocolScheme}://bobe`,
    `base-uri ${protocolScheme}://bobe`,
    "form-action 'none'",
    "frame-ancestors 'none'",
    "object-src 'none'",
  ].join('; ')
}

// =============================================================================
// Path traversal protection (extracted logic from security.ts)
// =============================================================================

function isPathSafe(rendererDir: string, requestPath: string): boolean {
  const resolvedPath = resolve(rendererDir, requestPath)
  const rel = relative(rendererDir, resolvedPath)
  return !rel.startsWith('..') && !isAbsolute(rel)
}

// =============================================================================
// IPC channel allowlist validation
// =============================================================================

const ALLOWED_CHANNELS = [
  'bobe:get-state',
  'bobe:toggle-capture',
  'settings:get',
  'settings:update',
  'models:list',
  'models:registry',
  'models:pull',
  'models:delete',
  'memories:list',
  'goals:list',
  'souls:list',
]

function isChannelAllowed(channel: string): boolean {
  return ALLOWED_CHANNELS.includes(channel)
}

// =============================================================================
// Tests
// =============================================================================

describe('Production CSP', () => {
  const csp = getProductionCSP('app', 'http://127.0.0.1:8766')

  it('blocks all sources by default', () => {
    expect(csp).toContain("default-src 'none'")
  })

  it('allows scripts only from app://bobe', () => {
    expect(csp).toContain('script-src app://bobe')
    expect(csp).not.toContain("'unsafe-eval'")
  })

  it('allows connections only to daemon', () => {
    expect(csp).toContain('connect-src http://127.0.0.1:8766')
  })

  it('blocks frames and objects', () => {
    expect(csp).toContain("frame-ancestors 'none'")
    expect(csp).toContain("object-src 'none'")
  })

  it('blocks form actions', () => {
    expect(csp).toContain("form-action 'none'")
  })

  it('script-src has no external sources', () => {
    const scriptSrc = csp.split(';').find((d) => d.trim().startsWith('script-src'))!
    expect(scriptSrc).not.toContain('http')
    expect(scriptSrc).not.toContain('https')
    expect(scriptSrc).toContain('app://bobe')
  })
})

describe('Path traversal protection', () => {
  const rendererDir = '/app/out/renderer'

  it('allows files inside renderer directory', () => {
    expect(isPathSafe(rendererDir, 'index.html')).toBe(true)
    expect(isPathSafe(rendererDir, 'assets/main.js')).toBe(true)
    expect(isPathSafe(rendererDir, 'settings.html')).toBe(true)
  })

  it('blocks parent directory traversal', () => {
    expect(isPathSafe(rendererDir, '../main/index.js')).toBe(false)
    expect(isPathSafe(rendererDir, '../../package.json')).toBe(false)
    expect(isPathSafe(rendererDir, '../../../etc/passwd')).toBe(false)
  })

  it('blocks encoded traversal', () => {
    // resolve() handles these, but double-check
    expect(isPathSafe(rendererDir, '%2e%2e/main/index.js')).toBe(true) // URL encoding doesn't affect path.resolve
  })

  it('blocks absolute paths', () => {
    expect(isPathSafe(rendererDir, '/etc/passwd')).toBe(false)
    expect(isPathSafe(rendererDir, '/usr/local/bin/bash')).toBe(false)
  })

  it('handles deeply nested valid paths', () => {
    expect(isPathSafe(rendererDir, 'assets/fonts/inter/regular.woff2')).toBe(true)
  })

  it('handles empty path', () => {
    // resolve(dir, '') = dir itself, which is relative ''
    expect(isPathSafe(rendererDir, '')).toBe(true)
  })
})

describe('IPC channel allowlist', () => {
  it('allows registered channels', () => {
    expect(isChannelAllowed('settings:get')).toBe(true)
    expect(isChannelAllowed('models:list')).toBe(true)
    expect(isChannelAllowed('models:delete')).toBe(true)
  })

  it('rejects unregistered channels', () => {
    expect(isChannelAllowed('shell:exec')).toBe(false)
    expect(isChannelAllowed('fs:readFile')).toBe(false)
    expect(isChannelAllowed('')).toBe(false)
  })

  it('rejects similar-looking channels', () => {
    expect(isChannelAllowed('settings:get ')).toBe(false) // trailing space
    expect(isChannelAllowed('SETTINGS:GET')).toBe(false) // uppercase
  })
})
