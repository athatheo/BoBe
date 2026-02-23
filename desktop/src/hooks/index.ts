/**
 * Hooks exports
 *
 * PRIMARY HOOKS:
 * - useBobe() - Full state + actions (use for most components)
 * - useBobeSelector() - Subscribe to state slice (use for performance)
 *
 * ARCHITECTURE:
 * - State lives in bobe-store.ts (single source of truth)
 * - Actions are plain functions, not hooks
 * - No hook nesting - flat dependency graph
 * - For loading states, use React 19 useTransition locally
 */

// Primary state/action hooks
export { useBobe, useBobeSelector, bobeActions } from './bobe-store'

// UI state hooks
export { useIndicatorState } from './useIndicatorState'
export type { IndicatorState } from './useIndicatorState'

// Re-export store utilities for testing/advanced use
export { getSnapshot, subscribe, initializeClient } from './bobe-store'

// Settings hooks
export { useTheme, initializeTheme } from './useTheme'
