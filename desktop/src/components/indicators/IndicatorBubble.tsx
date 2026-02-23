/**
 * Indicator bubble component
 *
 * Small bubble that appears next to the avatar showing transient states
 * like thinking, analyzing, and tool executions.
 *
 * Features:
 * - Debounced state transitions to prevent flickering
 * - Minimum display time for smooth UX
 * - Expandable history view for completed tools
 */

import { motion, AnimatePresence } from 'framer-motion'
import { Wrench, ChevronDown, ChevronUp, Check, X } from 'lucide-react'
import type { IndicatorType, ToolExecution } from '@/types/bobe'
import { useIndicatorState } from '@/hooks'
import { SPRING_CONFIG, INDICATOR_TIMING } from '@/lib/constants'

interface IndicatorBubbleProps {
  indicator: IndicatorType | null
  toolExecutions?: ToolExecution[]
}

/**
 * Convert snake_case tool name to readable format
 * e.g., "read_file" -> "read file"
 */
function formatToolName(name: string): string {
  return name.replace(/_/g, ' ')
}

export function IndicatorBubble({ indicator, toolExecutions = [] }: IndicatorBubbleProps) {
  const { displayIndicator, displayTools, toolHistory, isExpanded, toggleExpanded } =
    useIndicatorState({ indicator, toolExecutions })

  const hasRunningTools = displayTools.length > 0
  const hasHistory = toolHistory.length > 0

  // Show bubble when we have something to display
  const showBubble = displayIndicator !== null || hasRunningTools || (isExpanded && hasHistory)

  return (
    <AnimatePresence>
      {showBubble && (
        <motion.div
          className="indicator-bubble no-drag"
          initial={{ opacity: 0, x: 10, scale: 0.9 }}
          animate={{ opacity: 1, x: 0, scale: 1 }}
          exit={{ opacity: 0, x: 10, scale: 0.9 }}
          transition={{
            type: 'spring',
            ...SPRING_CONFIG,
          }}
        >
          <div className="indicator-bubble-content">
            {/* Main indicator row */}
            <div className="indicator-bubble-main">
              {hasRunningTools ? (
                <ToolExecutionContent tools={displayTools} />
              ) : (
                displayIndicator && <IndicatorContent indicator={displayIndicator} />
              )}

              {/* Expand button when there's history */}
              {(hasHistory || hasRunningTools) && (
                <button
                  className="indicator-expand-btn"
                  onClick={toggleExpanded}
                  aria-label={isExpanded ? 'Collapse history' : 'Expand history'}
                >
                  {isExpanded ? <ChevronUp size={10} /> : <ChevronDown size={10} />}
                </button>
              )}
            </div>

            {/* Expanded history view */}
            <AnimatePresence>
              {isExpanded && hasHistory && (
                <motion.div
                  className="indicator-history"
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: INDICATOR_TIMING.EXPAND_ANIMATION / 1000 }}
                >
                  <div className="indicator-history-list">
                    {toolHistory.slice(-5).map((tool) => (
                      <ToolHistoryItem key={tool.tool_call_id} tool={tool} />
                    ))}
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

interface IndicatorContentProps {
  indicator: IndicatorType
}

function IndicatorContent({ indicator }: IndicatorContentProps) {
  switch (indicator) {
    case 'thinking':
      return <ThinkingContent />
    case 'analyzing':
      return <AnalyzingContent />
    case 'idle':
    case 'capturing':
    case 'generating':
    case 'speaking':
      return null
    default: {
      // Exhaustive check - TypeScript will error if a case is missed
      const _exhaustive: never = indicator
      return null
    }
  }
}

function ThinkingContent() {
  return (
    <div className="indicator-bubble-row">
      <span className="indicator-bubble-text">thinking</span>
      <AnimatedDots />
    </div>
  )
}

function AnalyzingContent() {
  return (
    <div className="indicator-bubble-row">
      <span className="indicator-bubble-text">analyzing</span>
      <AnimatedDots />
    </div>
  )
}

function ToolExecutionContent({ tools }: { tools: ToolExecution[] }) {
  if (tools.length === 1) {
    return (
      <div className="indicator-bubble-row">
        <motion.div
          animate={{ rotate: 360 }}
          transition={{ duration: 2, repeat: Infinity, ease: 'linear' }}
        >
          <Wrench size={12} className="indicator-bubble-icon" />
        </motion.div>
        <span className="indicator-bubble-text">running {formatToolName(tools[0]!.tool_name)}</span>
        <AnimatedDots />
      </div>
    )
  }

  return (
    <div className="indicator-bubble-row">
      <motion.div
        animate={{ rotate: 360 }}
        transition={{ duration: 2, repeat: Infinity, ease: 'linear' }}
      >
        <Wrench size={12} className="indicator-bubble-icon" />
      </motion.div>
      <span className="indicator-bubble-text">running {tools.length} tools</span>
      <AnimatedDots />
    </div>
  )
}

function ToolHistoryItem({ tool }: { tool: ToolExecution }) {
  const isSuccess = tool.status === 'success'
  const duration = tool.duration_ms ? `${tool.duration_ms}ms` : ''

  return (
    <div className={`indicator-history-item ${isSuccess ? 'success' : 'error'}`}>
      {isSuccess ? (
        <Check size={10} className="indicator-history-icon success" />
      ) : (
        <X size={10} className="indicator-history-icon error" />
      )}
      <span className="indicator-history-name">{formatToolName(tool.tool_name)}</span>
      {duration && <span className="indicator-history-duration">{duration}</span>}
    </div>
  )
}

function AnimatedDots() {
  return (
    <div className="indicator-dots">
      {[0, 1, 2].map((i) => (
        <motion.span
          key={i}
          className="indicator-dot"
          animate={{ opacity: [0.3, 1, 0.3] }}
          transition={{
            duration: 1,
            repeat: Infinity,
            delay: i * 0.2,
            ease: 'easeInOut',
          }}
        >
          .
        </motion.span>
      ))}
    </div>
  )
}
