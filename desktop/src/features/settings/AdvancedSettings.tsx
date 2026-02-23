/**
 * AdvancedSettings component
 *
 * "For Nerds" section — similarity thresholds, goal check interval,
 * learning interval, conversation timeout, and MCP toggle.
 * User-facing settings moved to AIModelSettings and BehaviorSettings.
 */

import { useEffect, useState, useCallback, useRef } from 'react'
import { motion } from 'framer-motion'
import {
  Terminal,
  RefreshCw,
  AlertCircle,
  Target,
  Server,
  Brain,
  MessageSquare,
  Layers,
  FolderOpen,
} from 'lucide-react'
import { cn } from '@/lib/cn'
import type { DaemonSettings } from '@/types/api'

function getSettingsClient() {
  if (typeof window !== 'undefined' && 'settings' in window) {
    return window.settings
  }
  return null
}

export function AdvancedSettings() {
  const [settings, setSettings] = useState<DaemonSettings | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

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

  const updateSetting = useCallback(async (key: string, value: boolean | number | string) => {
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
  }, [])

  if (loading) {
    return (
      <div className="preferences-panel">
        <div className="preferences-loading">
          <RefreshCw size={24} className="animate-spin" />
          <span>Loading daemon settings...</span>
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

  if (!settings) {
    return (
      <div className="preferences-panel">
        <div className="preferences-empty">
          <Terminal size={32} />
          <p>Connect to daemon to view settings</p>
        </div>
      </div>
    )
  }

  return (
    <div className="preferences-panel">
      {error && (
        <div className="preferences-error" style={{ marginBottom: '1rem' }}>
          <AlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      {/* Similarity Thresholds */}
      <SettingsSection
        icon={<Layers size={20} />}
        title="Similarity Thresholds"
        description="Vector similarity thresholds for memory operations"
      >
        <DebouncedDecimal
          label="Deduplication"
          description="Threshold for detecting duplicate memories"
          value={settings.similarity_deduplication_threshold}
          min={0}
          max={1}
          step={0.01}
          onCommit={(v) => updateSetting('similarity_deduplication_threshold', v)}
          disabled={saving}
        />
        <DebouncedDecimal
          label="Search recall"
          description="Minimum similarity for memory retrieval"
          value={settings.similarity_search_recall_threshold}
          min={0}
          max={1}
          step={0.01}
          onCommit={(v) => updateSetting('similarity_search_recall_threshold', v)}
          disabled={saving}
        />
        <DebouncedDecimal
          label="Clustering"
          description="Similarity for memory clustering"
          value={settings.similarity_clustering_threshold}
          min={0}
          max={1}
          step={0.01}
          onCommit={(v) => updateSetting('similarity_clustering_threshold', v)}
          disabled={saving}
        />
      </SettingsSection>

      {/* Goal Check */}
      <SettingsSection
        icon={<Target size={20} />}
        title="Goals"
        description="Goal tracking intervals"
      >
        <DebouncedNumber
          label="Check interval"
          description="Seconds between goal relevance checks"
          value={Math.round(settings.goal_check_interval_seconds)}
          min={60}
          max={7200}
          onCommit={(v) => updateSetting('goal_check_interval_seconds', v)}
          disabled={saving}
        />
      </SettingsSection>

      {/* Learning */}
      <SettingsSection
        icon={<Brain size={20} />}
        title="Learning"
        description="Background learning cycle timing"
      >
        <DebouncedNumber
          label="Learning interval"
          description="Minutes between learning cycles"
          value={settings.learning_interval_minutes}
          min={1}
          max={1440}
          onCommit={(v) => updateSetting('learning_interval_minutes', v)}
          disabled={saving || !settings.learning_enabled}
        />
      </SettingsSection>

      {/* Conversation Timeout */}
      <SettingsSection
        icon={<MessageSquare size={20} />}
        title="Conversation"
        description="Advanced conversation timing"
      >
        <DebouncedNumber
          label="Inactivity timeout"
          description="Seconds before allowing new proactive reachout"
          value={settings.conversation_inactivity_timeout_seconds}
          min={5}
          max={600}
          onCommit={(v) => updateSetting('conversation_inactivity_timeout_seconds', v)}
          disabled={saving}
        />
      </SettingsSection>

      {/* Projects */}
      <SettingsSection icon={<FolderOpen size={20} />} title="Projects" description="Default directory where BoBe creates project folders from goals">
        <DirectoryPicker
          value={settings.projects_directory ?? ''}
          onCommit={(v) => updateSetting('projects_directory', v)}
          disabled={saving}
        />
      </SettingsSection>

      {/* MCP */}
      <SettingsSection
        icon={<Server size={20} />}
        title="MCP Protocol"
        description="Model Context Protocol server connections"
      >
        <SettingToggle
          label="Enable MCP"
          description="Connect to MCP servers for extended capabilities"
          checked={settings.mcp_enabled}
          onChange={(v) => updateSetting('mcp_enabled', v)}
          disabled={saving}
        />
      </SettingsSection>
    </div>
  )
}

// =============================================================================
// SECTION
// =============================================================================

function SettingsSection({
  icon,
  title,
  description,
  children,
}: {
  icon: React.ReactNode
  title: string
  description: string
  children: React.ReactNode
}) {
  return (
    <div className="preferences-section">
      <div className="preferences-section-header">
        <span className="preferences-section-icon">{icon}</span>
        <div>
          <h3 className="preferences-section-title">{title}</h3>
          <p className="preferences-section-description">{description}</p>
        </div>
      </div>
      <div className="settings-controls">{children}</div>
    </div>
  )
}

// =============================================================================
// TOGGLE
// =============================================================================

function SettingToggle({
  label,
  description,
  checked,
  onChange,
  disabled,
}: {
  label: string
  description: string
  checked: boolean
  onChange: (value: boolean) => void
  disabled?: boolean
}) {
  return (
    <label className={cn('settings-toggle-row', disabled && 'settings-toggle-disabled')}>
      <div className="settings-toggle-info">
        <span className="settings-toggle-label">{label}</span>
        <span className="settings-toggle-description">{description}</span>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => !disabled && onChange(!checked)}
        className={cn('settings-toggle', checked && 'settings-toggle-on')}
        disabled={disabled}
      >
        <motion.span
          className="settings-toggle-thumb"
          animate={{ x: checked ? 18 : 2 }}
          transition={{ type: 'spring', stiffness: 500, damping: 30 }}
        />
      </button>
    </label>
  )
}

// =============================================================================
// DEBOUNCED NUMBER INPUT
// =============================================================================

function DebouncedNumber({
  label,
  description,
  value,
  min,
  max,
  onCommit,
  disabled,
}: {
  label: string
  description: string
  value: number
  min: number
  max: number
  onCommit: (value: number) => void
  disabled?: boolean
}) {
  const [localValue, setLocalValue] = useState(String(value))
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    setLocalValue(String(value))
  }, [value])
  useEffect(
    () => () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    },
    [],
  )

  function commitValue(raw: string) {
    const v = parseInt(raw, 10)
    if (!isNaN(v) && v >= min && v <= max && v !== value) onCommit(v)
  }

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const raw = e.target.value
    setLocalValue(raw)
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => commitValue(raw), 600)
  }

  function handleBlur() {
    if (timerRef.current) clearTimeout(timerRef.current)
    commitValue(localValue)
  }

  return (
    <div className={cn('settings-number-row', disabled && 'settings-number-disabled')}>
      <div className="settings-toggle-info">
        <span className="settings-toggle-label">{label}</span>
        <span className="settings-toggle-description">{description}</span>
      </div>
      <input
        type="number"
        value={localValue}
        min={min}
        max={max}
        onChange={handleChange}
        onBlur={handleBlur}
        className="settings-number-input"
        disabled={disabled}
      />
    </div>
  )
}

// =============================================================================
// DIRECTORY PICKER
// =============================================================================

function DirectoryPicker({ value, onCommit, disabled }: {
  value: string; onCommit: (value: string) => void; disabled?: boolean
}) {
  const [localValue, setLocalValue] = useState(value)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const isElectron = typeof window !== 'undefined' && 'settings' in window && 'selectDirectory' in (window.settings ?? {})

  useEffect(() => { setLocalValue(value) }, [value])
  useEffect(() => () => { if (timerRef.current) clearTimeout(timerRef.current) }, [])

  function commitValue(raw: string) {
    if (raw !== value) onCommit(raw)
  }

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const raw = e.target.value
    setLocalValue(raw)
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => commitValue(raw), 600)
  }

  function handleBlur() {
    if (timerRef.current) clearTimeout(timerRef.current)
    commitValue(localValue)
  }

  async function handleBrowse() {
    const path = await window.settings.selectDirectory()
    if (path) {
      setLocalValue(path)
      if (timerRef.current) clearTimeout(timerRef.current)
      onCommit(path)
    }
  }

  return (
    <div className={cn('settings-directory-row', disabled && 'settings-number-disabled')}>
      <div className="settings-directory-control">
        <input
          type="text"
          value={localValue}
          onChange={handleChange}
          onBlur={handleBlur}
          className="settings-directory-input"
          placeholder="/path/to/projects"
          disabled={disabled}
        />
        {isElectron && (
          <button
            type="button"
            onClick={handleBrowse}
            className="settings-directory-browse"
            disabled={disabled}
          >
            Browse
          </button>
        )}
      </div>
    </div>
  )
}

// =============================================================================
// DEBOUNCED DECIMAL INPUT (for similarity thresholds)
// =============================================================================

function DebouncedDecimal({
  label,
  description,
  value,
  min,
  max,
  step,
  onCommit,
  disabled,
}: {
  label: string
  description: string
  value: number
  min: number
  max: number
  step: number
  onCommit: (value: number) => void
  disabled?: boolean
}) {
  const [localValue, setLocalValue] = useState(String(value))
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    setLocalValue(String(value))
  }, [value])
  useEffect(
    () => () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    },
    [],
  )

  function commitValue(raw: string) {
    const v = parseFloat(raw)
    if (!isNaN(v) && v >= min && v <= max && v !== value) onCommit(v)
  }

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const raw = e.target.value
    setLocalValue(raw)
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => commitValue(raw), 600)
  }

  function handleBlur() {
    if (timerRef.current) clearTimeout(timerRef.current)
    commitValue(localValue)
  }

  return (
    <div className={cn('settings-number-row', disabled && 'settings-number-disabled')}>
      <div className="settings-toggle-info">
        <span className="settings-toggle-label">{label}</span>
        <span className="settings-toggle-description">{description}</span>
      </div>
      <input
        type="number"
        value={localValue}
        min={min}
        max={max}
        step={step}
        onChange={handleChange}
        onBlur={handleBlur}
        className="settings-number-input"
        disabled={disabled}
      />
    </div>
  )
}
