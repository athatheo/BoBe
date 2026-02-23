/**
 * Playwright configuration for Electron E2E testing
 *
 * Note: This config is for Electron testing, not browser testing.
 * We don't use browser projects - instead we launch the Electron app directly.
 */

import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './e2e/tests',
  testMatch: '**/*.spec.ts',

  // Timeout for each test
  timeout: 30_000,

  // Timeout for expect assertions
  expect: {
    timeout: 5_000,
  },

  // Fail the build on CI if you accidentally left test.only in the source code
  forbidOnly: !!process.env.CI,

  // Retry on CI only
  retries: process.env.CI ? 2 : 0,

  // Use single worker for Electron tests to avoid system-level conflicts
  workers: 1,

  // Reporter configuration
  reporter: process.env.CI ? 'github' : 'list',

  // Single project for Electron tests (no browser projects needed)
  projects: [
    {
      name: 'electron',
      testMatch: '**/*.spec.ts',
    },
  ],

  // Global setup/teardown if needed later
  // globalSetup: './e2e/global-setup.ts',

  use: {
    // Collect trace on first retry
    trace: 'on-first-retry',

    // Screenshot on failure
    screenshot: 'only-on-failure',
  },

  // Output directory for test artifacts
  outputDir: 'e2e/test-results',
})
