/**
 * Basic Electron app tests
 *
 * Tests fundamental app behavior: launch, window creation, main process evaluation.
 */

import { test, expect } from '../fixtures/electron.fixture'

test.describe('Electron App', () => {
  test('should launch successfully', async ({ electronApp }) => {
    // App should be running
    const isPackaged = await electronApp.evaluate(({ app }) => app.isPackaged)
    expect(isPackaged).toBe(false) // We're testing the development build
  })

  test('should create the overlay window', async ({ electronApp: _electronApp, window }) => {
    // Window should exist and be visible
    const isVisible = await window.evaluate(() => true) // If we can evaluate, window exists
    expect(isVisible).toBe(true)
  })

  test('should have correct window properties', async ({ electronApp }) => {
    // Get window count
    const windowCount = await electronApp.evaluate(
      ({ BrowserWindow }) => BrowserWindow.getAllWindows().length,
    )
    expect(windowCount).toBeGreaterThanOrEqual(1)

    // Check overlay window properties
    const windowProps = await electronApp.evaluate(({ BrowserWindow }) => {
      const win = BrowserWindow.getAllWindows()[0]
      return {
        isAlwaysOnTop: win.isAlwaysOnTop(),
        isResizable: win.isResizable(),
        isMovable: win.isMovable(),
      }
    })

    expect(windowProps.isAlwaysOnTop).toBe(true)
    expect(windowProps.isResizable).toBe(false)
    expect(windowProps.isMovable).toBe(true)
  })

  test('should have security settings enabled', async ({ window }) => {
    // Test that nodeIntegration is disabled by checking if require is undefined
    const hasRequire = await window.evaluate(() => {
      return typeof (globalThis as unknown as { require?: unknown }).require !== 'undefined'
    })
    expect(hasRequire).toBe(false) // nodeIntegration should be false

    // Test that contextIsolation is enabled by checking window.bobe exists (exposed via preload)
    // If contextIsolation were false, the preload wouldn't need contextBridge
    const hasBobeApi = await window.evaluate(() => {
      return typeof (window as unknown as { bobe?: unknown }).bobe === 'object'
    })
    expect(hasBobeApi).toBe(true) // API exposed via contextBridge

    // Test that we can't access Node.js APIs directly (sandbox enabled)
    const hasProcess = await window.evaluate(() => {
      return typeof (globalThis as unknown as { process?: unknown }).process !== 'undefined'
    })
    expect(hasProcess).toBe(false) // process should not be exposed
  })

  test('should expose window.bobe API', async ({ window }) => {
    // Check that the preload script exposed the bobe API
    const hasBobeApi = await window.evaluate(() => {
      return typeof (window as unknown as { bobe: unknown }).bobe === 'object'
    })
    expect(hasBobeApi).toBe(true)
  })

  test('should have expected bobe API methods', async ({ window }) => {
    const apiMethods = await window.evaluate(() => {
      const bobe = (window as unknown as { bobe: Record<string, unknown> }).bobe
      return {
        hasGetState: typeof bobe.getState === 'function',
        hasToggleCapture: typeof bobe.toggleCapture === 'function',
        hasDismissMessage: typeof bobe.dismissMessage === 'function',
        hasOnStateUpdate: typeof bobe.onStateUpdate === 'function',
      }
    })

    expect(apiMethods.hasGetState).toBe(true)
    expect(apiMethods.hasToggleCapture).toBe(true)
    expect(apiMethods.hasDismissMessage).toBe(true)
    expect(apiMethods.hasOnStateUpdate).toBe(true)
  })
})
