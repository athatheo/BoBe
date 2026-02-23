/**
 * Main avatar component
 *
 * Displays the BoBe avatar with state indicators and controls.
 * Uses Tailwind CSS for styling with custom CSS variables from globals.css.
 */

import { useState, useEffect, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { MessageCircle } from 'lucide-react'
import { StateIndicator, ThinkingNumbers, SpeakingWave } from '@/components/indicators'
import { cn } from '@/lib/cn'
import type { BobeStateType } from '@/types/bobe'

// Status explanations for each state (single words)
const STATUS_LABELS: Record<BobeStateType, string> = {
  loading: 'Loading',
  idle: 'Ready',
  capturing: 'Capturing',
  thinking: 'Thinking',
  speaking: 'Speaking',
  wants_to_speak: 'Hey',
}

// =============================================================================
// TYPES
// =============================================================================

interface AvatarProps {
  stateType: BobeStateType
  isCapturing: boolean
  isConnected: boolean
  hasMessage: boolean
  showInput?: boolean
  onClick?: () => void
  onToggleCapture?: () => void
  onToggleInput?: () => void
}

// =============================================================================
// MAIN COMPONENT
// =============================================================================

export function Avatar({
  stateType,
  isCapturing: _isCapturing,
  isConnected,
  hasMessage,
  showInput = false,
  onClick,
  onToggleCapture: _onToggleCapture,
  onToggleInput,
}: AvatarProps) {
  return (
    <div className="avatar-container" data-testid="avatar">
      {/* Circular avatar card */}
      <div className="avatar-card">
        {/* Status label - appears on state change, half in/out at top */}
        <StatusLabel state={stateType} />

        {/* Thinking numbers - orbits in the ring gap (only when thinking) */}
        {stateType === 'thinking' && <ThinkingNumbers />}

        {/* Speaking wave - sound modulation (only when speaking) */}
        {stateType === 'speaking' && <SpeakingWave />}

        {/* Attention pulse - ring glows when wants to speak */}
        {stateType === 'wants_to_speak' && <AttentionPulse />}

        {/* Main avatar circle */}
        <motion.div
          className="avatar-inner no-drag"
          whileHover={{ scale: 1.06 }}
          whileTap={{ scale: 0.96 }}
          onClick={onClick}
        >
          {/* Inner circle - warm, matte gradient */}
          <div className="avatar-circle" />

          {/* State indicator - uses unified switch pattern */}
          <div className="avatar-indicator-container">
            <StateIndicator state={stateType} chatOpen={showInput} />
          </div>

          {/* Connection dot */}
          <ConnectionDot connected={isConnected} />

          {/* Message notification badge */}
          {hasMessage && <MessageBadge />}
        </motion.div>

        {/* Chat toggle - positioned at bottom left */}
        <ChatToggle isActive={showInput} onToggle={onToggleInput} />
      </div>

      {/* BoBe label - positioned below the avatar card in natural flow */}
      <BobeLabel />
    </div>
  )
}

// =============================================================================
// SUB-COMPONENTS
// =============================================================================

interface ConnectionDotProps {
  connected: boolean
}

function ConnectionDot({ connected }: ConnectionDotProps) {
  return (
    <div
      className={cn(
        'connection-dot',
        connected ? 'connection-dot-connected' : 'connection-dot-disconnected',
      )}
    />
  )
}

function MessageBadge() {
  return (
    <motion.div
      className="message-badge"
      initial={{ scale: 0 }}
      animate={{ scale: [1, 1.1, 1] }}
      transition={{
        duration: 2,
        repeat: Infinity,
        ease: 'easeInOut',
      }}
    />
  )
}

interface StatusLabelProps {
  state: BobeStateType
}

function StatusLabel({ state }: StatusLabelProps) {
  const [text, setText] = useState('')
  // Track the state we're displaying (for animation keys and minimum time)
  const [displayState, setDisplayState] = useState<BobeStateType | null>(null)
  // Track when current state started showing (for minimum display time)
  const shownAtRef = useRef<number | null>(null)
  // Track pending state change (waiting for minimum time)
  const pendingStateRef = useRef<BobeStateType | null>(null)

  const MINIMUM_DISPLAY_MS = 2000

  useEffect(() => {
    const now = Date.now()

    // Skip idle state - don't show label
    if (state === 'idle') {
      // If we're showing something, check minimum time
      if (displayState && shownAtRef.current) {
        const elapsed = now - shownAtRef.current
        if (elapsed < MINIMUM_DISPLAY_MS) {
          // Wait for minimum time then hide
          pendingStateRef.current = null // null means hide
          const remaining = MINIMUM_DISPLAY_MS - elapsed
          const timer = setTimeout(() => {
            setDisplayState(null)
            shownAtRef.current = null
          }, remaining)
          return () => clearTimeout(timer)
        }
      }
      setDisplayState(null)
      shownAtRef.current = null
      return
    }

    // Same state, nothing to do
    if (state === displayState) {
      return
    }

    // New state - check if we need to wait for minimum time
    if (displayState && shownAtRef.current) {
      const elapsed = now - shownAtRef.current
      if (elapsed < MINIMUM_DISPLAY_MS) {
        // Wait for minimum time then switch
        pendingStateRef.current = state
        const remaining = MINIMUM_DISPLAY_MS - elapsed
        const timer = setTimeout(() => {
          const pending = pendingStateRef.current
          if (pending) {
            setText('')
            setDisplayState(pending)
            shownAtRef.current = Date.now()
            pendingStateRef.current = null
          }
        }, remaining)
        return () => clearTimeout(timer)
      }
    }

    // Show new state immediately
    setText('')
    setDisplayState(state)
    shownAtRef.current = now
  }, [state, displayState])

  // Typewriter effect for text
  useEffect(() => {
    if (!displayState) return

    const fullText = STATUS_LABELS[displayState]
    let i = 0
    let isCancelled = false

    const typeTimer = setInterval(() => {
      if (isCancelled) return
      i++
      setText(fullText.slice(0, i))
      if (i >= fullText.length) {
        clearInterval(typeTimer)
      }
    }, 40)

    return () => {
      isCancelled = true
      clearInterval(typeTimer)
    }
  }, [displayState])

  // Use AnimatePresence to handle exit animations properly
  return (
    <AnimatePresence mode="wait">
      {displayState && (
        <motion.div
          key={displayState}
          className="status-label"
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -4 }}
          transition={{ duration: 0.25 }}
        >
          <span className="status-label-text">{text}</span>
          <span className="status-label-cursor">|</span>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

function BobeLabel() {
  return (
    <div className="bobe-label">
      <span className="bobe-label-text">BoBe</span>
    </div>
  )
}

function AttentionPulse() {
  return (
    <motion.div
      className="attention-pulse"
      animate={{
        scale: [1, 1.08, 1],
        opacity: [0.7, 1, 0.7],
      }}
      transition={{
        duration: 1.5,
        repeat: Infinity,
        ease: 'easeInOut',
      }}
    />
  )
}

interface ChatToggleProps {
  isActive: boolean
  onToggle?: () => void
}

function ChatToggle({ isActive, onToggle }: ChatToggleProps) {
  return (
    <motion.button
      className={cn('chat-toggle no-drag', isActive && 'chat-toggle-active')}
      whileHover={{ scale: 1.08 }}
      whileTap={{ scale: 0.95 }}
      onClick={(e) => {
        e.stopPropagation()
        onToggle?.()
      }}
      title="Send message"
    >
      <MessageCircle size={14} strokeWidth={2.5} />
    </motion.button>
  )
}
