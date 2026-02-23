/**
 * Shared form controls for settings panels.
 *
 * Section/Row/Toggle/Select/RangeSlider — reusable controls for
 * BehaviorSettings and other settings panels.
 */

import { useEffect, useState, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { cn } from '@/lib/cn'

// =============================================================================
// Section — header with toggle, collapsible details
// =============================================================================

export function Section({
  icon,
  title,
  desc,
  enabled,
  onToggle,
  saving,
  children,
}: {
  icon: React.ReactNode
  title: string
  desc: string
  enabled?: boolean
  onToggle?: (v: boolean) => void
  saving?: boolean
  children: React.ReactNode
}) {
  const hasToggle = onToggle !== undefined
  const isOpen = hasToggle ? enabled : true

  return (
    <div className={cn('bhv-section', hasToggle && !enabled && 'bhv-section-off')}>
      <div className="bhv-header">
        <div className="bhv-header-left">
          <span className="bhv-icon">{icon}</span>
          <div>
            <span className="bhv-title">{title}</span>
            <span className="bhv-desc">{desc}</span>
          </div>
        </div>
        {hasToggle && (
          <Toggle checked={!!enabled} onChange={(v) => !saving && onToggle!(v)} disabled={saving} />
        )}
      </div>
      <AnimatePresence initial={false}>
        {isOpen && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            style={{ overflow: 'hidden' }}
          >
            <div className="bhv-body">{children}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

// =============================================================================
// Row — label + control
// =============================================================================

export function Row({
  label,
  suffix,
  children,
}: {
  label: string
  suffix?: string
  children: React.ReactNode
}) {
  return (
    <div className="bhv-row">
      <span className="bhv-label">{label}</span>
      <div className="bhv-control">
        {children}
        {suffix && <span className="bhv-suffix">{suffix}</span>}
      </div>
    </div>
  )
}

// =============================================================================
// Toggle switch
// =============================================================================

export function Toggle({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      className={cn('bhv-toggle', checked && 'bhv-toggle-on')}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation()
        if (!disabled) onChange(!checked)
      }}
    >
      <motion.span
        className="bhv-toggle-thumb"
        animate={{ x: checked ? 18 : 2 }}
        transition={{ type: 'spring', stiffness: 500, damping: 30 }}
      />
    </button>
  )
}

// =============================================================================
// Select dropdown
// =============================================================================

export function Select({
  value,
  options,
  onChange,
  disabled,
}: {
  value: string
  options: ReadonlyArray<{ value: string; label: string; disabled?: boolean }>
  onChange: (v: string) => void
  disabled?: boolean
}) {
  return (
    <select
      className="bhv-select"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      disabled={disabled}
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value} disabled={opt.disabled}>
          {opt.label}
        </option>
      ))}
    </select>
  )
}

// =============================================================================
// Range slider
// =============================================================================

export function RangeSlider({
  value,
  min,
  max,
  step,
  onCommit,
  disabled,
}: {
  value: number
  min: number
  max: number
  step: number
  onCommit: (v: number) => void
  disabled?: boolean
}) {
  const [local, setLocal] = useState(value)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    setLocal(value)
  }, [value])
  useEffect(
    () => () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    },
    [],
  )

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const v = parseFloat(e.target.value)
    setLocal(v)
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => {
      if (v !== value) onCommit(v)
    }, 300)
  }

  return (
    <input
      type="range"
      className="bhv-range"
      value={local}
      min={min}
      max={max}
      step={step}
      onChange={handleChange}
      disabled={disabled}
    />
  )
}

// =============================================================================
// Helpers
// =============================================================================

export function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}
