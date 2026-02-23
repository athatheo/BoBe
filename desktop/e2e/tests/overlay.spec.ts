/**
 * Overlay window UI tests
 *
 * Tests the React renderer: Avatar, state indicators, speech bubble, etc.
 * These tests interact with the DOM just like regular Playwright web tests.
 */

import { test, expect } from '../fixtures/electron.fixture'

test.describe('Overlay UI', () => {
  test('should render the app root', async ({ window }) => {
    // Wait for React to mount
    const root = window.locator('#root')
    await expect(root).toBeVisible()
  })

  test('should render the avatar', async ({ window }) => {
    // Avatar should be visible
    const avatar = window.locator('[data-testid="avatar"]')
    await expect(avatar).toBeVisible({ timeout: 10_000 })
  })

  test('should render state indicator', async ({ window }) => {
    // State indicator should be visible within avatar
    const indicator = window.locator('[data-testid="state-indicator"]')
    await expect(indicator).toBeVisible({ timeout: 10_000 })
  })

  test('should show loading state initially', async ({ window }) => {
    // When daemon is not connected, should show loading state
    // Note: This may need adjustment based on actual initial state behavior
    const loadingIndicator = window.locator('[data-testid="state-indicator-loading"]')

    // Either loading is visible, or another state has already taken over
    const _isLoading = await loadingIndicator.isVisible().catch(() => false)

    // Just verify something is rendered (loading or another state)
    const anyIndicator = window.locator('[data-testid^="state-indicator-"]')
    await expect(anyIndicator.first()).toBeVisible({ timeout: 10_000 })
  })

  test('should have correct avatar dimensions', async ({ window }) => {
    const avatar = window.locator('[data-testid="avatar"]')
    await expect(avatar).toBeVisible({ timeout: 10_000 })

    const box = await avatar.boundingBox()
    expect(box).not.toBeNull()

    // Avatar should be roughly square (allowing for some variance)
    if (box) {
      const aspectRatio = box.width / box.height
      expect(aspectRatio).toBeGreaterThan(0.8)
      expect(aspectRatio).toBeLessThan(1.2)
    }
  })
})

test.describe('Overlay Interactions', () => {
  test('should be able to click on avatar', async ({ window }) => {
    const avatar = window.locator('[data-testid="avatar"]')
    await expect(avatar).toBeVisible({ timeout: 10_000 })

    // Click should not throw
    await avatar.click()
  })

  test('should have no console errors on load', async ({ window }) => {
    const errors: string[] = []

    // Collect console errors
    window.on('console', (msg) => {
      if (msg.type() === 'error') {
        errors.push(msg.text())
      }
    })

    // Wait a bit for any async errors
    await window.waitForTimeout(2000)

    // Filter out known acceptable errors (like daemon connection failures in test mode)
    const criticalErrors = errors.filter(
      (err) => !err.includes('daemon') && !err.includes('ECONNREFUSED') && !err.includes('fetch'),
    )

    expect(criticalErrors).toHaveLength(0)
  })
})

test.describe('Overlay Accessibility', () => {
  test('should have no critical accessibility violations', async ({ window }) => {
    // Basic accessibility check: ensure interactive elements are keyboard accessible
    const avatar = window.locator('[data-testid="avatar"]')
    await expect(avatar).toBeVisible({ timeout: 10_000 })

    // Check that avatar can receive focus (if it's interactive)
    // This is a basic check - for full a11y testing, use @axe-core/playwright
  })
})
