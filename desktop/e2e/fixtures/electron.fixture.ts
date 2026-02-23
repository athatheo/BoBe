/**
 * Electron test fixture for Playwright
 *
 * Provides reusable fixtures for launching and testing the Electron app.
 * Tests run against the built app (out/ directory), not the dev server.
 *
 * Usage:
 *   import { test, expect } from '../fixtures/electron.fixture'
 *
 *   test('my test', async ({ electronApp, window }) => {
 *     // electronApp: ElectronApplication instance
 *     // window: First window (Page instance) - use for React/renderer testing
 *   })
 */

import { test as base, _electron as electron } from '@playwright/test'
import type { ElectronApplication, Page } from '@playwright/test'
import { resolve } from 'node:path'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'

// Path to the built main process entry point
const MAIN_ENTRY = resolve(__dirname, '../../out/main/index.js')

/**
 * Custom fixtures for Electron testing
 */
type ElectronFixtures = {
  /** The Electron application instance */
  electronApp: ElectronApplication
  /** The first (overlay) window - a Playwright Page for testing the React UI */
  window: Page
}

/**
 * Extended test with Electron fixtures
 */
export const test = base.extend<ElectronFixtures>({
  // Launch Electron app before each test
  electronApp: [
    async ({}, use) => {
      // Create a unique temporary directory for this test to avoid SingletonLock conflicts
      const userDataDir = mkdtempSync(resolve(tmpdir(), 'bobe-test-'))

      const electronApp = await electron.launch({
        args: [MAIN_ENTRY, `--user-data-dir=${userDataDir}`],
        env: {
          ...process.env,
          NODE_ENV: 'test',
          // Disable daemon connection during tests to avoid external dependencies
          MAIN_VITE_TESTING: 'true',
        },
      })

      // Forward main process console to test output (helpful for debugging)
      electronApp.on('console', async (msg) => {
        const values = await Promise.all(
          msg.args().map((arg) => arg.jsonValue().catch(() => '[unserializable]')),
        )
        console.log(`[Main Process] ${msg.type()}:`, ...values)
      })

      // Wait for the first window to ensure the app is fully started
      // This prevents "execution context destroyed" errors
      const firstWindow = await electronApp.firstWindow()
      await firstWindow.waitForLoadState('domcontentloaded')

      await use(electronApp)

      // Cleanup: close app after test with a timeout
      try {
        await Promise.race([
          electronApp.close(),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error('App close timeout')), 5000),
          ),
        ])
      } catch {
        // Force kill if close times out
        try {
          const pid = electronApp.process().pid
          if (pid) process.kill(pid, 'SIGKILL')
        } catch {
          // Ignore kill errors
        }
      }

      // Clean up the temp user data directory
      try {
        rmSync(userDataDir, { recursive: true, force: true })
      } catch {
        // Ignore cleanup errors
      }
    },
    { timeout: 60000 }, // Increase fixture timeout
  ],

  // Get the first window (overlay window) for React/renderer testing
  window: async ({ electronApp }, use) => {
    // Get the first window (should already be open from electronApp fixture)
    const window = await electronApp.firstWindow()

    // Ensure it's fully loaded
    await window.waitForLoadState('domcontentloaded')

    await use(window)
  },
})

/**
 * Re-export expect from Playwright
 */
export { expect } from '@playwright/test'

/**
 * Helper to evaluate code in the main Electron process
 *
 * @example
 * const appPath = await evaluateInMain(electronApp, ({ app }) => app.getAppPath())
 */
export async function evaluateInMain<T>(
  electronApp: ElectronApplication,
  fn: (modules: { app: Electron.App; BrowserWindow: typeof Electron.BrowserWindow }) => T,
): Promise<T> {
  return electronApp.evaluate(fn)
}
