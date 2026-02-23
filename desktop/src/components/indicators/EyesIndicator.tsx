/**
 * Minimalistic eyes indicator
 *
 * Simple eye design that conveys state through expression:
 * - Loading: Attentive open eyes
 * - Idle: Closed/sleeping eyes
 * - Capturing: Half-lidded eyes
 * - Thinking: Eyes looking around
 * - Speaking: Open eyes with animation
 * - Wants to speak: Wide open eyes
 */

import { useState, useEffect } from 'react'
import { motion } from 'framer-motion'
import type { BobeStateType } from '@/types/bobe'

// Show attentive (open) eyes instead of sleeping for the first 5 minutes after startup
const WARMUP_DURATION_MS = 5 * 60 * 1000
const APP_START_TIME = Date.now()

interface EyesIndicatorProps {
  state: BobeStateType
  chatOpen?: boolean
}

export function EyesIndicator({ state, chatOpen = false }: EyesIndicatorProps) {
  // Track warmup period (first 5 minutes after app startup)
  const [isWarmup, setIsWarmup] = useState(() => Date.now() - APP_START_TIME < WARMUP_DURATION_MS)

  useEffect(() => {
    if (!isWarmup) return
    const remaining = WARMUP_DURATION_MS - (Date.now() - APP_START_TIME)
    if (remaining <= 0) {
      setIsWarmup(false)
      return
    }
    const timer = setTimeout(() => setIsWarmup(false), remaining)
    return () => clearTimeout(timer)
  }, [isWarmup])

  // When chat is open or in warmup period, show attentive eyes instead of sleeping
  if ((chatOpen || isWarmup) && state === 'idle') {
    return <AttentiveEyes />
  }

  switch (state) {
    case 'loading':
      return <AttentiveEyes />
    case 'idle':
      return <SleepingEyes />
    case 'capturing':
      return <CapturingEyes />
    case 'thinking':
      return <ThinkingEyes />
    case 'speaking':
      return <SpeakingEyes />
    case 'wants_to_speak':
      return <EagerEyes />
    default: {
      const _exhaustive: never = state
      return <SleepingEyes />
    }
  }
}

// Sleeping eyes - simple curved lines (◡ ◡)
function SleepingEyes() {
  return (
    <div className="eyes-container" data-testid="state-indicator-idle">
      <svg width="36" height="20" viewBox="0 0 36 20">
        {/* Left eye - closed */}
        <path
          d="M4 10 Q9 14, 14 10"
          fill="none"
          stroke="var(--color-bobe-charcoal)"
          strokeWidth="2"
          strokeLinecap="round"
          opacity="0.6"
        />
        {/* Right eye - closed */}
        <path
          d="M22 10 Q27 14, 32 10"
          fill="none"
          stroke="var(--color-bobe-charcoal)"
          strokeWidth="2"
          strokeLinecap="round"
          opacity="0.6"
        />
      </svg>
    </div>
  )
}

// Capturing eyes - eyes scanning with viewfinder brackets
function CapturingEyes() {
  return (
    <div className="eyes-container" data-testid="state-indicator-capturing">
      <svg width="40" height="24" viewBox="0 0 40 24">
        {/* Viewfinder corner brackets */}
        <motion.g
          stroke="var(--color-bobe-charcoal)"
          strokeWidth="1.5"
          fill="none"
          strokeLinecap="round"
          animate={{ opacity: [0.4, 0.8, 0.4] }}
          transition={{ duration: 2, repeat: Infinity, ease: 'easeInOut' }}
        >
          <path d="M2 6 L2 2 L6 2" />
          <path d="M34 2 L38 2 L38 6" />
          <path d="M2 18 L2 22 L6 22" />
          <path d="M34 22 L38 22 L38 18" />
        </motion.g>

        {/* Eye outline (white for contrast with avatar face) */}
        <ellipse
          cx="12"
          cy="12"
          rx="5.5"
          ry="5"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        <ellipse
          cx="28"
          cy="12"
          rx="5.5"
          ry="5"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        {/* Sclera (white) */}
        <ellipse cx="12" cy="12" rx="4.5" ry="4" fill="white" />
        <ellipse cx="28" cy="12" rx="4.5" ry="4" fill="white" />

        {/* Iris + Pupil - scanning */}
        <motion.g
          animate={{ x: [-1.5, 1.5, -1.5] }}
          transition={{ duration: 2.5, repeat: Infinity, ease: 'easeInOut' }}
        >
          <circle cx="12" cy="12" r="2.5" fill="var(--color-avatar-iris)" />
          <circle cx="28" cy="12" r="2.5" fill="var(--color-avatar-iris)" />
          <circle cx="12" cy="12" r="1" fill="var(--color-bobe-charcoal)" />
          <circle cx="28" cy="12" r="1" fill="var(--color-bobe-charcoal)" />
        </motion.g>
      </svg>
    </div>
  )
}

// Thinking eyes - just the face, numbers are rendered separately at avatar-card level
function ThinkingEyes() {
  return (
    <div className="eyes-container" data-testid="state-indicator-thinking">
      <svg width="36" height="24" viewBox="0 0 36 24">
        {/* Eye outline (white for contrast) */}
        <ellipse
          cx="9"
          cy="9"
          rx="6.5"
          ry="5.5"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        <ellipse
          cx="27"
          cy="9"
          rx="6.5"
          ry="5.5"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        {/* Sclera (white) */}
        <ellipse cx="9" cy="9" rx="5.5" ry="4.5" fill="white" />
        <ellipse cx="27" cy="9" rx="5.5" ry="4.5" fill="white" />

        {/* Iris + Pupil - looking up */}
        <motion.g
          animate={{ y: [-1, -0.5, -1] }}
          transition={{ duration: 2, repeat: Infinity, ease: 'easeInOut' }}
        >
          <circle cx="9" cy="8" r="3" fill="var(--color-avatar-iris)" />
          <circle cx="27" cy="8" r="3" fill="var(--color-avatar-iris)" />
          <circle cx="9" cy="8" r="1.2" fill="var(--color-bobe-charcoal)" />
          <circle cx="27" cy="8" r="1.2" fill="var(--color-bobe-charcoal)" />
        </motion.g>

        {/* Thinking mouth */}
        <ellipse
          cx="18"
          cy="19"
          rx="2.5"
          ry="2"
          fill="none"
          stroke="var(--color-avatar-mouth)"
          strokeWidth="2"
        />
      </svg>
    </div>
  )
}

// Floating numbers that bubble up in the ring gap - rendered at avatar-card level
export function ThinkingNumbers() {
  // Just the characters - positions will be randomized
  const chars = ['1', '+', '0', '=', 'π', '7', '%', '∑', '×', '2', '/', '9']

  return (
    <div className="thinking-numbers-ring">
      {chars.map((char, i) => (
        <BubblingChar key={i} char={char} index={i} />
      ))}
    </div>
  )
}

// Individual bubbling character with randomized position
function BubblingChar({ char, index }: { char: string; index: number }) {
  // Randomize x position within the ring gap (-45 to 45)
  const randomX = () => Math.random() * 90 - 45
  const [xPos, setXPos] = useState(randomX)

  return (
    <motion.span
      className="thinking-number-char"
      style={{
        position: 'absolute',
        left: '50%',
        top: '50%',
      }}
      animate={{
        x: [xPos, xPos + (Math.random() * 10 - 5)],
        y: [30, -40],
        opacity: [0, 1, 1, 0],
      }}
      transition={{
        duration: 2.5 + (index % 3) * 0.3,
        repeat: Infinity,
        ease: 'easeOut',
        delay: index * 0.4,
        times: [0, 0.2, 0.8, 1],
      }}
      onAnimationComplete={() => setXPos(randomX())}
    >
      {char}
    </motion.span>
  )
}

// Sound wave bars for speaking state - rendered at avatar-card level
export function SpeakingWave() {
  // Sound wave bars around the ring
  const bars = Array.from({ length: 16 }, (_, i) => ({
    angle: (i * 360) / 16,
    delay: i * 0.08,
  }))

  return (
    <div className="speaking-wave-ring">
      {bars.map((bar, i) => {
        const rad = (bar.angle * Math.PI) / 180
        const radius = 52
        const x = Math.cos(rad) * radius
        const y = Math.sin(rad) * radius

        return (
          <motion.div
            key={i}
            className="speaking-wave-bar"
            style={{
              position: 'absolute',
              left: '50%',
              top: '50%',
              transform: `translate(${x - 2}px, ${y - 6}px) rotate(${bar.angle + 90}deg)`,
            }}
            animate={{
              scaleY: [0.3, 1, 0.5, 0.8, 0.3],
            }}
            transition={{
              duration: 0.6,
              repeat: Infinity,
              ease: 'easeInOut',
              delay: bar.delay,
            }}
          />
        )
      })}
    </div>
  )
}

// Speaking eyes - friendly eyes with animated talking mouth
function SpeakingEyes() {
  return (
    <div className="eyes-container" data-testid="state-indicator-speaking">
      <svg width="36" height="28" viewBox="0 0 36 28">
        {/* Eye outline (white for contrast) */}
        <ellipse
          cx="9"
          cy="9"
          rx="6.5"
          ry="5.5"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        <ellipse
          cx="27"
          cy="9"
          rx="6.5"
          ry="5.5"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        {/* Sclera (white) */}
        <ellipse cx="9" cy="9" rx="5.5" ry="4.5" fill="white" />
        <ellipse cx="27" cy="9" rx="5.5" ry="4.5" fill="white" />

        {/* Iris */}
        <circle cx="9" cy="9" r="3" fill="var(--color-avatar-iris)" />
        <circle cx="27" cy="9" r="3" fill="var(--color-avatar-iris)" />
        {/* Pupil */}
        <circle cx="9" cy="9" r="1.2" fill="var(--color-bobe-charcoal)" />
        <circle cx="27" cy="9" r="1.2" fill="var(--color-bobe-charcoal)" />

        {/* Highlight dots */}
        <circle cx="7.5" cy="7.5" r="1" fill="white" />
        <circle cx="25.5" cy="7.5" r="1" fill="white" />

        {/* Animated talking mouth - scale from center */}
        <motion.ellipse
          cx="18"
          cy="21"
          rx="4"
          ry="2"
          fill="var(--color-avatar-mouth)"
          style={{ transformOrigin: '18px 21px', transformBox: 'fill-box' }}
          animate={{ scaleY: [1, 1.75, 0.75, 1.5, 1] }}
          transition={{ duration: 0.5, repeat: Infinity, ease: 'easeInOut' }}
        />
      </svg>
    </div>
  )
}

// Eager eyes - wants to speak, excited/attentive look
function EagerEyes() {
  return (
    <div className="eyes-container" data-testid="state-indicator-wants_to_speak">
      <svg width="36" height="28" viewBox="0 0 36 28">
        {/* Raised eyebrows - dark */}
        <motion.path
          d="M4 4 Q9 2, 14 4"
          fill="none"
          stroke="var(--color-bobe-charcoal)"
          strokeWidth="2"
          strokeLinecap="round"
          animate={{ y: [0, -1, 0] }}
          transition={{ duration: 1, repeat: Infinity, ease: 'easeInOut' }}
        />
        <motion.path
          d="M22 4 Q27 2, 32 4"
          fill="none"
          stroke="var(--color-bobe-charcoal)"
          strokeWidth="2"
          strokeLinecap="round"
          animate={{ y: [0, -1, 0] }}
          transition={{ duration: 1, repeat: Infinity, ease: 'easeInOut', delay: 0.1 }}
        />

        {/* Eye outline (white for contrast) */}
        <ellipse
          cx="9"
          cy="12"
          rx="6.5"
          ry="6"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        <ellipse
          cx="27"
          cy="12"
          rx="6.5"
          ry="6"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        {/* Sclera (white) */}
        <ellipse cx="9" cy="12" rx="5.5" ry="5" fill="white" />
        <ellipse cx="27" cy="12" rx="5.5" ry="5" fill="white" />

        {/* Iris + Pupil */}
        <motion.g
          animate={{ y: [0, -0.5, 0] }}
          transition={{ duration: 1.5, repeat: Infinity, ease: 'easeInOut' }}
        >
          <circle cx="9" cy="11.5" r="3.5" fill="var(--color-avatar-iris)" />
          <circle cx="27" cy="11.5" r="3.5" fill="var(--color-avatar-iris)" />
          <circle cx="9" cy="11.5" r="1.5" fill="var(--color-bobe-charcoal)" />
          <circle cx="27" cy="11.5" r="1.5" fill="var(--color-bobe-charcoal)" />
        </motion.g>

        {/* Highlight dots */}
        <circle cx="7" cy="10" r="1.2" fill="white" />
        <circle cx="25" cy="10" r="1.2" fill="white" />

        {/* Excited smile */}
        <path
          d="M14 23 Q18 26, 22 23"
          fill="none"
          stroke="var(--color-avatar-mouth)"
          strokeWidth="2"
          strokeLinecap="round"
        />
      </svg>
    </div>
  )
}

// Attentive eyes - open eyes looking at user (for when chat is open)
function AttentiveEyes() {
  return (
    <div className="eyes-container" data-testid="state-indicator-attentive">
      <motion.svg
        width="36"
        height="20"
        viewBox="0 0 36 20"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.2 }}
      >
        {/* Eye outline (white for contrast) */}
        <ellipse
          cx="9"
          cy="10"
          rx="6.5"
          ry="5.5"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        <ellipse
          cx="27"
          cy="10"
          rx="6.5"
          ry="5.5"
          fill="none"
          stroke="var(--color-avatar-eye-outline)"
          strokeWidth="1.5"
        />
        {/* Sclera (white) */}
        <ellipse cx="9" cy="10" rx="5.5" ry="4.5" fill="white" />
        <ellipse cx="27" cy="10" rx="5.5" ry="4.5" fill="white" />

        {/* Iris + Pupil */}
        <motion.g
          animate={{ y: [0, -0.3, 0] }}
          transition={{ duration: 2, repeat: Infinity, ease: 'easeInOut' }}
        >
          <circle cx="9" cy="10" r="3" fill="var(--color-avatar-iris)" />
          <circle cx="27" cy="10" r="3" fill="var(--color-avatar-iris)" />
          <circle cx="9" cy="10" r="1.2" fill="var(--color-bobe-charcoal)" />
          <circle cx="27" cy="10" r="1.2" fill="var(--color-bobe-charcoal)" />
        </motion.g>

        {/* Highlight dots */}
        <circle cx="7.5" cy="8.5" r="1" fill="white" />
        <circle cx="25.5" cy="8.5" r="1" fill="white" />
      </motion.svg>
    </div>
  )
}
