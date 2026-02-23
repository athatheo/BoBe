/**
 * BehaviorSettings component
 *
 * Clean flat-list settings layout inspired by macOS System Preferences.
 * Sections separated by dividers. Toggle in header row, details below.
 */

import { useEffect, useState, useCallback, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Camera,
  Clock,
  Brain,
  MessageSquare,
  Wrench,
  RefreshCw,
  AlertCircle,
  Plus,
  X,
  Check,
} from 'lucide-react'
import { cn } from '@/lib/cn'
import type { DaemonSettings } from '@/types/api'
import type { MediaAccessStatus } from '@/types/ipc'

function getSettingsClient() {
  if (typeof window !== 'undefined' && 'settings' in window) {
    return window.settings
  }
  return null
}

export function BehaviorSettings() {
  const [settings, setSettings] = useState<DaemonSettings | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [screenPermission, setScreenPermission] = useState<MediaAccessStatus | null>(null)

  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    loadSettings()
    return () => {
      mountedRef.current = false
    }
  }, [])

  async function loadSettings() {
    setLoading(true)
    setError(null)
    try {
      const client = getSettingsClient()
      if (!client) {
        setError('Settings API not available')
        return
      }
      const data = await client.get()
      if (mountedRef.current) setSettings(data)
    } catch (err) {
      if (mountedRef.current)
        setError(err instanceof Error ? err.message : 'Failed to load settings')
    } finally {
      if (mountedRef.current) setLoading(false)
    }
  }

  const updateSetting = useCallback(
    async (key: string, value: boolean | number | string | string[]) => {
      setSaving(true)
      setError(null)
      let previousValue: unknown
      setSettings((prev) => {
        if (!prev) return prev
        previousValue = prev[key as keyof DaemonSettings]
        return { ...prev, [key]: value } as DaemonSettings
      })
      try {
        const client = getSettingsClient()
        if (client) await client.update({ [key]: value })
      } catch (err) {
        setSettings((prev) => {
          if (!prev) return prev
          return { ...prev, [key]: previousValue } as DaemonSettings
        })
        setError(err instanceof Error ? err.message : 'Failed to save setting')
      } finally {
        setSaving(false)
      }
    },
    [],
  )

  const handleCaptureToggle = useCallback(
    async (enabled: boolean) => {
      if (!enabled) {
        // Turning off always allowed
        updateSetting('capture_enabled', false)
        setScreenPermission(null)
        return
      }
      // Turning on — check screen recording permission first
      if (typeof window !== 'undefined' && 'permissions' in window && window.permissions) {
        const status = await window.permissions.checkScreen()
        setScreenPermission(status)
        if (status !== 'granted') return
      }
      updateSetting('capture_enabled', true)
    },
    [updateSetting],
  )

  if (loading) {
    return (
      <div className="preferences-panel">
        <div className="preferences-loading">
          <RefreshCw size={24} className="animate-spin" />
          <span>Loading behavior settings...</span>
        </div>
      </div>
    )
  }

  if (error && !settings) {
    return (
      <div className="preferences-panel">
        <div className="preferences-error">
          <AlertCircle size={24} />
          <span>{error}</span>
          <button onClick={loadSettings} className="preferences-retry-btn">
            Retry
          </button>
        </div>
      </div>
    )
  }

  if (!settings) return null

  return (
    <div className="bhv">
      {error && (
        <div className="preferences-error" style={{ margin: '0 0 16px', padding: '10px 14px' }}>
          <AlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      {/* Screen Capture */}
      <Section
        icon={<Camera size={16} />}
        title="Screen Capture"
        desc="BoBe periodically captures your screen for context"
        enabled={settings.capture_enabled}
        onToggle={handleCaptureToggle}
        saving={saving}
      >
        {screenPermission && screenPermission !== 'granted' && (
          <div className="bhv-permission-msg">
            <AlertCircle size={13} />
            <span>Screen recording permission required.</span>
            <button
              className="bhv-permission-link"
              onClick={() => window.permissions?.openScreenSettings()}
            >
              Open System Settings
            </button>
            <span className="bhv-permission-note">Restart BoBe after granting.</span>
          </div>
        )}
        <Row label="Capture every" suffix="seconds">
          <NumInput
            value={settings.capture_interval_seconds}
            min={1}
            max={600}
            onCommit={(v) => updateSetting('capture_interval_seconds', v)}
            disabled={saving}
          />
        </Row>
      </Section>

      {/* Check-ins */}
      <Section
        icon={<Clock size={16} />}
        title="Check-ins"
        desc="Scheduled proactive check-ins throughout the day"
        enabled={settings.checkin_enabled}
        onToggle={(v) => updateSetting('checkin_enabled', v)}
        saving={saving}
      >
        <div className="bhv-row">
          <span className="bhv-label">Schedule</span>
          <TimePills
            times={settings.checkin_times}
            onChange={(t) => updateSetting('checkin_times', t)}
            disabled={saving}
          />
        </div>
        <Row label="Jitter" suffix="minutes">
          <NumInput
            value={settings.checkin_jitter_minutes}
            min={0}
            max={30}
            onCommit={(v) => updateSetting('checkin_jitter_minutes', v)}
            disabled={saving}
          />
        </Row>
      </Section>

      {/* Memory */}
      <Section
        icon={<Brain size={16} />}
        title="Memory"
        desc="How long BoBe retains memories"
        enabled={settings.learning_enabled}
        onToggle={(v) => updateSetting('learning_enabled', v)}
        saving={saving}
      >
        <Row label="Short-term retention" suffix="days">
          <NumInput
            value={settings.memory_short_term_retention_days}
            min={1}
            max={365}
            onCommit={(v) => updateSetting('memory_short_term_retention_days', v)}
            disabled={saving}
          />
        </Row>
        <Row label="Long-term retention" suffix="days">
          <NumInput
            value={settings.memory_long_term_retention_days}
            min={1}
            max={3650}
            onCommit={(v) => updateSetting('memory_long_term_retention_days', v)}
            disabled={saving}
          />
        </Row>
      </Section>

      {/* Conversation */}
      <Section
        icon={<MessageSquare size={16} />}
        title="Conversation"
        desc="How conversations are managed"
      >
        <Row label="Auto-close after" suffix="minutes">
          <NumInput
            value={settings.conversation_auto_close_minutes}
            min={1}
            max={60}
            onCommit={(v) => updateSetting('conversation_auto_close_minutes', v)}
            disabled={saving}
          />
        </Row>
        <div className="bhv-row">
          <span className="bhv-label">Generate summaries</span>
          <Toggle
            checked={settings.conversation_summary_enabled}
            onChange={(v) => updateSetting('conversation_summary_enabled', v)}
            disabled={saving}
          />
        </div>
      </Section>

      {/* Tools */}
      <Section
        icon={<Wrench size={16} />}
        title="Tools"
        desc="Allow BoBe to execute actions on your behalf"
        enabled={settings.tools_enabled}
        onToggle={(v) => updateSetting('tools_enabled', v)}
        saving={saving}
      >
        <Row label="Max iterations" suffix="rounds">
          <NumInput
            value={settings.tools_max_iterations}
            min={1}
            max={20}
            onCommit={(v) => updateSetting('tools_max_iterations', v)}
            disabled={saving}
          />
        </Row>
      </Section>
    </div>
  )
}

// =============================================================================
// Section — header with toggle, collapsible details
// =============================================================================

function Section({
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

function Row({
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

function Toggle({
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
// Number input
// =============================================================================

function NumInput({
  value,
  min,
  max,
  onCommit,
  disabled,
}: {
  value: number
  min: number
  max: number
  onCommit: (v: number) => void
  disabled?: boolean
}) {
  const [local, setLocal] = useState(String(value))
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    setLocal(String(value))
  }, [value])
  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current)
    },
    [],
  )

  function commit(raw: string) {
    const v = parseInt(raw, 10)
    if (!isNaN(v) && v >= min && v <= max && v !== value) onCommit(v)
  }

  return (
    <input
      type="number"
      className="bhv-num"
      value={local}
      min={min}
      max={max}
      disabled={disabled}
      onChange={(e) => {
        setLocal(e.target.value)
        if (timer.current) clearTimeout(timer.current)
        timer.current = setTimeout(() => commit(e.target.value), 600)
      }}
      onBlur={() => {
        if (timer.current) clearTimeout(timer.current)
        commit(local)
      }}
    />
  )
}

// =============================================================================
// Time pills
// =============================================================================

function TimePills({
  times,
  onChange,
  disabled,
}: {
  times: string[]
  onChange: (t: string[]) => void
  disabled?: boolean
}) {
  const [adding, setAdding] = useState(false)
  const [newTime, setNewTime] = useState('09:00')

  function add() {
    if (!newTime) return
    onChange([...new Set([...times, newTime])].sort())
    setAdding(false)
    setNewTime('09:00')
  }

  return (
    <div className="bhv-pills">
      {times.map((t) => (
        <span key={t} className="bhv-pill">
          {t}
          {!disabled && (
            <button
              className="bhv-pill-x"
              onClick={() => onChange(times.filter((x) => x !== t))}
              type="button"
            >
              <X size={10} />
            </button>
          )}
        </span>
      ))}
      {!adding ? (
        <button
          className="bhv-pill-add"
          onClick={() => setAdding(true)}
          disabled={disabled}
          type="button"
        >
          <Plus size={11} />
        </button>
      ) : (
        <span className="bhv-pill-input-wrap">
          <input
            type="time"
            value={newTime}
            onChange={(e) => setNewTime(e.target.value)}
            className="bhv-pill-input"
            autoFocus
            onKeyDown={(e) => {
              if (e.key === 'Enter') add()
              if (e.key === 'Escape') setAdding(false)
            }}
          />
          <button className="bhv-pill-ok" onClick={add} type="button">
            <Check size={11} />
          </button>
          <button className="bhv-pill-cancel" onClick={() => setAdding(false)} type="button">
            <X size={11} />
          </button>
        </span>
      )}
    </div>
  )
}
