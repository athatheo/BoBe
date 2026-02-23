/**
 * Application constants
 *
 * Design tokens are defined in globals.css via Tailwind @theme.
 * These constants are for JS-side values that can't be in CSS
 * (animation timing for Framer Motion, window dimensions, etc.)
 */

/**
 * Spring configuration for Framer Motion transitions
 * Used by speech bubbles, chat bubbles, indicators, etc.
 */
export const SPRING_CONFIG = {
  damping: 20,
  stiffness: 300,
  mass: 0.8,
} as const

/**
 * Window sizing constants - shared between renderer and electron main process
 *
 * Content dimensions:
 * - Avatar card: 116px x 116px
 * - Chat bubbles: 248px width
 * - Container padding: 12px
 * - Status label overflow: 14px above avatar
 * - BobeLabel overflow: 11px below avatar
 */
export const WINDOW_SIZES = {
  WIDTH_COLLAPSED: 148,
  WIDTH_EXPANDED: 340,
  HEIGHT_COLLAPSED: 180,
  HEIGHT_AVATAR: 180,
  HEIGHT_INPUT: 70,
  HEIGHT_MESSAGE: 110,
  HEIGHT_MAX: 700,
  MARGIN: 16,
} as const

/**
 * Indicator timing constants
 * Based on UX research for preventing flickering and ensuring smooth state transitions:
 * - < 300ms: Don't show indicator (appears instant)
 * - 300ms delay: Wait before showing to avoid flash for fast operations
 * - 500-600ms minimum: Once shown, keep visible to prevent stuttering
 */
export const INDICATOR_TIMING = {
  /** Wait before showing any indicator to avoid flash for fast operations */
  DELAY_BEFORE_SHOW: 300,
  /** Minimum time to display indicator once shown to prevent stuttering */
  MIN_DISPLAY_TIME: 600,
  /** Show "thinking" for at least this before transitioning to tool indicators */
  THINKING_MIN_TIME: 800,
  /** How long to show completed tool before removing from display */
  TOOL_COMPLETE_LINGER: 1500,
  /** Animation duration for expand/collapse transitions */
  EXPAND_ANIMATION: 200,
} as const
