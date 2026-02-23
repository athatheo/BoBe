/**
 * Indicator state management hook
 *
 * Handles UX smoothing for rapid state changes to prevent flickering:
 * - Delays showing indicators for fast operations (< 300ms)
 * - Ensures minimum display time once shown (600ms)
 * - Shows "thinking" for minimum time before transitioning to tools
 * - Tracks tool execution history for expandable view
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react'
import type { IndicatorType, ToolExecution } from '@/types/bobe'
import { INDICATOR_TIMING } from '@/lib/constants'

export interface IndicatorState {
  /** Current indicator to display (null if hidden) */
  displayIndicator: IndicatorType | null
  /** Tools currently shown as running */
  displayTools: ToolExecution[]
  /** Recent tool history (completed tools for expanded view) */
  toolHistory: ToolExecution[]
  /** Whether expanded view is shown */
  isExpanded: boolean
  /** Toggle expanded view */
  toggleExpanded: () => void
  /** Whether we're in a coalesced "multiple tools" state */
  isCoalesced: boolean
}

interface IndicatorStateOptions {
  indicator: IndicatorType | null
  toolExecutions: ToolExecution[]
}

/**
 * Hook that smooths indicator state transitions to prevent flickering
 */
export function useIndicatorState({
  indicator,
  toolExecutions,
}: IndicatorStateOptions): IndicatorState {
  // Display state (what we actually show, after smoothing)
  const [displayIndicator, setDisplayIndicator] = useState<IndicatorType | null>(null)
  const [displayTools, setDisplayTools] = useState<ToolExecution[]>([])
  const [toolHistory, setToolHistory] = useState<ToolExecution[]>([])
  const [isExpanded, setIsExpanded] = useState(false)

  // Timing refs
  const showTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const indicatorShownAtRef = useRef<number | null>(null)
  const thinkingShownAtRef = useRef<number | null>(null)

  // Refs to access latest values in timer callbacks (avoids stale closures)
  const latestIndicatorRef = useRef<IndicatorType | null>(indicator)
  const latestToolExecutionsRef = useRef<ToolExecution[]>(toolExecutions)

  // Keep refs up to date
  useEffect(() => {
    latestIndicatorRef.current = indicator
    latestToolExecutionsRef.current = toolExecutions
  }, [indicator, toolExecutions])

  // Memoize running tools to avoid dependency array issues
  const runningTools = useMemo(
    () => toolExecutions.filter((t) => t.status === 'running'),
    [toolExecutions],
  )

  // Create stable key for running tools comparison
  const runningToolsKey = useMemo(
    () => runningTools.map((t) => t.tool_call_id).join(','),
    [runningTools],
  )

  const hasRunningTools = runningTools.length > 0

  // Determine if we should show something (before smoothing)
  const shouldShow = (indicator && ['thinking', 'analyzing'].includes(indicator)) || hasRunningTools

  // Handle indicator changes with debouncing
  useEffect(() => {
    const now = Date.now()

    // Clear any pending show timer
    if (showTimerRef.current) {
      clearTimeout(showTimerRef.current)
      showTimerRef.current = null
    }

    if (shouldShow) {
      // Clear any pending hide timer
      if (hideTimerRef.current) {
        clearTimeout(hideTimerRef.current)
        hideTimerRef.current = null
      }

      // If already showing something, update immediately (no delay for transitions)
      if (displayIndicator !== null) {
        // Check if we need to honor minimum thinking time before showing tools
        if (
          thinkingShownAtRef.current &&
          hasRunningTools &&
          now - thinkingShownAtRef.current < INDICATOR_TIMING.THINKING_MIN_TIME
        ) {
          // Still in minimum thinking time, delay tool display
          const remainingTime =
            INDICATOR_TIMING.THINKING_MIN_TIME - (now - thinkingShownAtRef.current)
          showTimerRef.current = setTimeout(() => {
            // Use ref to get latest value, avoiding stale closure
            const currentRunning = latestToolExecutionsRef.current.filter(
              (t) => t.status === 'running',
            )
            setDisplayTools(currentRunning)
          }, remainingTime)
        } else {
          // Update tools immediately
          setDisplayTools(runningTools)
          if (indicator && indicator !== 'thinking') {
            setDisplayIndicator(indicator)
          }
        }
        return
      }

      // Delay before showing new indicator (prevents flash for fast operations)
      showTimerRef.current = setTimeout(() => {
        // Use refs to get latest values, avoiding stale closures
        const currentIndicator = latestIndicatorRef.current
        const currentRunning = latestToolExecutionsRef.current.filter((t) => t.status === 'running')

        indicatorShownAtRef.current = Date.now()
        if (currentIndicator === 'thinking' || currentIndicator === 'analyzing') {
          thinkingShownAtRef.current = Date.now()
        }
        setDisplayIndicator(currentIndicator)
        setDisplayTools(currentRunning)
      }, INDICATOR_TIMING.DELAY_BEFORE_SHOW)
    } else {
      // Should hide - but honor minimum display time
      if (indicatorShownAtRef.current) {
        const displayedFor = now - indicatorShownAtRef.current
        const remainingMinTime = INDICATOR_TIMING.MIN_DISPLAY_TIME - displayedFor

        if (remainingMinTime > 0) {
          // Wait for minimum display time
          hideTimerRef.current = setTimeout(() => {
            setDisplayIndicator(null)
            setDisplayTools([])
            indicatorShownAtRef.current = null
            thinkingShownAtRef.current = null
          }, remainingMinTime)
        } else {
          // Already shown long enough, hide immediately
          setDisplayIndicator(null)
          setDisplayTools([])
          indicatorShownAtRef.current = null
          thinkingShownAtRef.current = null
        }
      } else {
        // Never shown, just clear
        setDisplayIndicator(null)
        setDisplayTools([])
      }
    }

    // Cleanup on unmount or before next effect run
    return () => {
      if (showTimerRef.current) {
        clearTimeout(showTimerRef.current)
        showTimerRef.current = null
      }
      if (hideTimerRef.current) {
        clearTimeout(hideTimerRef.current)
        hideTimerRef.current = null
      }
    }
  }, [indicator, shouldShow, hasRunningTools, displayIndicator, runningToolsKey, runningTools])

  // Clear all refs on unmount to prevent stale state on remount
  useEffect(() => {
    return () => {
      indicatorShownAtRef.current = null
      thinkingShownAtRef.current = null
    }
  }, [])

  // Track tool history (completed tools)
  useEffect(() => {
    const completedTools = toolExecutions.filter(
      (t) => t.status === 'success' || t.status === 'error',
    )

    // Add new completed tools to history
    setToolHistory((prev) => {
      const existingIds = new Set(prev.map((t) => t.tool_call_id))
      const newTools = completedTools.filter((t) => !existingIds.has(t.tool_call_id))

      if (newTools.length === 0) return prev

      // Keep last 10 tools in history
      return [...prev, ...newTools].slice(-10)
    })
  }, [toolExecutions])

  // Clear history when indicator goes away for a while
  useEffect(() => {
    if (!shouldShow && toolHistory.length > 0) {
      const clearTimer = setTimeout(() => {
        setToolHistory([])
        setIsExpanded(false)
      }, 5000) // Clear history after 5s of inactivity

      return () => clearTimeout(clearTimer)
    }
  }, [shouldShow, toolHistory.length])

  const toggleExpanded = useCallback(() => {
    setIsExpanded((prev) => !prev)
  }, [])

  // Determine if we're coalescing multiple tools into summary
  const isCoalesced = displayTools.length > 2

  return {
    displayIndicator,
    displayTools,
    toolHistory,
    isExpanded,
    toggleExpanded,
    isCoalesced,
  }
}
