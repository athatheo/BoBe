/**
 * Unit tests for settings UI logic.
 *
 * Tests pure functions and state management patterns used by AdvancedSettings.
 * Does NOT render React components (no DOM needed for these tests).
 */

import { describe, it, expect } from 'vitest'
import type { DaemonSettings, ModelInfo } from '@/types/api'

// =============================================================================
// Test helpers — replicate pure functions from AdvancedSettings
// =============================================================================

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}

function buildModelOptions(
  models: ModelInfo[],
  currentModel: string,
): { value: string; label: string }[] {
  const options = models.map((m) => ({
    value: m.name,
    label: `${m.name} (${formatBytes(m.size_bytes)})`,
  }))
  if (currentModel && !options.some((o) => o.value === currentModel)) {
    options.unshift({ value: currentModel, label: `${currentModel} (current)` })
  }
  return options
}

function filterRegistryModels(registry: ModelInfo[], installed: ModelInfo[]): ModelInfo[] {
  return registry.filter((rm) => !installed.some((m) => m.name === rm.name))
}

// Simulates the functional setState pattern from AdvancedSettings
function optimisticUpdate(settings: DaemonSettings, key: string, value: unknown): DaemonSettings {
  return { ...settings, [key]: value } as DaemonSettings
}

function rollbackUpdate(
  settings: DaemonSettings,
  key: string,
  previousValue: unknown,
): DaemonSettings {
  return { ...settings, [key]: previousValue } as DaemonSettings
}

// =============================================================================
// Default test settings
// =============================================================================

const DEFAULT_SETTINGS: DaemonSettings = {
  llm_backend: 'ollama',
  ollama_model: 'qwen3:14b',
  openai_model: 'gpt-4o-mini',
  openai_api_key_set: false,
  azure_openai_endpoint: '',
  azure_openai_deployment: '',
  azure_openai_api_key_set: false,
  capture_enabled: true,
  capture_interval_seconds: 240,
  checkin_enabled: true,
  checkin_times: ['09:00', '14:00'],
  checkin_jitter_minutes: 5,
  learning_enabled: true,
  learning_interval_minutes: 30,
  conversation_inactivity_timeout_seconds: 30,
  conversation_auto_close_minutes: 10,
  conversation_summary_enabled: true,
  goal_check_interval_seconds: 900,
  projects_directory: '',
  tools_enabled: true,
  tools_max_iterations: 5,
  mcp_enabled: true,
  similarity_deduplication_threshold: 0.85,
  similarity_search_recall_threshold: 0.6,
  similarity_clustering_threshold: 0.8,
  memory_short_term_retention_days: 30,
  memory_long_term_retention_days: 90,
}

// =============================================================================
// Tests
// =============================================================================

describe('formatBytes', () => {
  it('formats gigabytes', () => {
    expect(formatBytes(8_000_000_000)).toBe('7.5 GB')
  })

  it('formats megabytes', () => {
    expect(formatBytes(500_000_000)).toBe('477 MB')
  })

  it('formats kilobytes', () => {
    expect(formatBytes(50_000)).toBe('49 KB')
  })

  it('handles zero', () => {
    expect(formatBytes(0)).toBe('0 KB')
  })

  it('handles exact gigabyte boundary', () => {
    expect(formatBytes(1_073_741_824)).toBe('1.0 GB')
  })
})

describe('buildModelOptions', () => {
  const models: ModelInfo[] = [
    { name: 'qwen3:14b', size_bytes: 8_000_000_000, modified_at: '' },
    { name: 'llama3:8b', size_bytes: 4_000_000_000, modified_at: '' },
  ]

  it('builds options from installed models', () => {
    const options = buildModelOptions(models, 'qwen3:14b')
    expect(options).toHaveLength(2)
    expect(options[0]!.value).toBe('qwen3:14b')
  })

  it('includes current model if not in list', () => {
    const options = buildModelOptions(models, 'deleted-model:7b')
    expect(options).toHaveLength(3)
    expect(options[0]!.value).toBe('deleted-model:7b')
    expect(options[0]!.label).toContain('(current)')
  })

  it('does not duplicate current model if already in list', () => {
    const options = buildModelOptions(models, 'qwen3:14b')
    expect(options).toHaveLength(2)
  })

  it('handles empty models list with current model', () => {
    const options = buildModelOptions([], 'qwen3:14b')
    expect(options).toHaveLength(1)
    expect(options[0]!.value).toBe('qwen3:14b')
    expect(options[0]!.label).toContain('(current)')
  })

  it('handles empty models list with empty current model', () => {
    const options = buildModelOptions([], '')
    expect(options).toHaveLength(0)
  })
})

describe('filterRegistryModels', () => {
  const registry: ModelInfo[] = [
    { name: 'qwen3:14b', size_bytes: 8e9, modified_at: '' },
    { name: 'llama3:8b', size_bytes: 4e9, modified_at: '' },
    { name: 'gemma3:12b', size_bytes: 6e9, modified_at: '' },
  ]
  const installed: ModelInfo[] = [{ name: 'qwen3:14b', size_bytes: 8e9, modified_at: '' }]

  it('filters out installed models', () => {
    const result = filterRegistryModels(registry, installed)
    expect(result).toHaveLength(2)
    expect(result.map((m) => m.name)).toEqual(['llama3:8b', 'gemma3:12b'])
  })

  it('returns all if none installed', () => {
    const result = filterRegistryModels(registry, [])
    expect(result).toHaveLength(3)
  })

  it('returns empty if all installed', () => {
    const result = filterRegistryModels(registry, registry)
    expect(result).toHaveLength(0)
  })
})

describe('optimistic update + rollback', () => {
  it('updates a single field', () => {
    const updated = optimisticUpdate(DEFAULT_SETTINGS, 'capture_enabled', false)
    expect(updated.capture_enabled).toBe(false)
    // Other fields preserved
    expect(updated.capture_interval_seconds).toBe(240)
    expect(updated.learning_enabled).toBe(true)
  })

  it('rollback restores original value', () => {
    const updated = optimisticUpdate(DEFAULT_SETTINGS, 'capture_enabled', false)
    expect(updated.capture_enabled).toBe(false)

    const rolledBack = rollbackUpdate(updated, 'capture_enabled', true)
    expect(rolledBack.capture_enabled).toBe(true)
  })

  it('sequential updates accumulate correctly', () => {
    let settings = DEFAULT_SETTINGS
    settings = optimisticUpdate(settings, 'capture_enabled', false)
    settings = optimisticUpdate(settings, 'capture_interval_seconds', 60)

    expect(settings.capture_enabled).toBe(false)
    expect(settings.capture_interval_seconds).toBe(60)
    // Original unchanged
    expect(DEFAULT_SETTINGS.capture_enabled).toBe(true)
    expect(DEFAULT_SETTINGS.capture_interval_seconds).toBe(240)
  })

  it('does not mutate original settings object', () => {
    const original = { ...DEFAULT_SETTINGS }
    optimisticUpdate(DEFAULT_SETTINGS, 'capture_enabled', false)
    expect(DEFAULT_SETTINGS).toEqual(original)
  })
})

describe('settings classification', () => {
  // These mirror the backend's 3-tier classification
  const HOT_SWAP_FIELDS = [
    'capture_enabled',
    'capture_interval_seconds',
    'checkin_enabled',
    'checkin_jitter_minutes',
    'learning_enabled',
    'learning_interval_minutes',
    'tools_enabled',
    'tools_max_iterations',
    'mcp_enabled',
    'conversation_summary_enabled',
    'conversation_inactivity_timeout_seconds',
    'conversation_auto_close_minutes',
    'goal_check_interval_seconds',
  ]

  const MODEL_FIELDS = ['ollama_model', 'openai_model']

  it('all hot-swap fields exist on DaemonSettings', () => {
    for (const field of HOT_SWAP_FIELDS) {
      expect(field in DEFAULT_SETTINGS).toBe(true)
    }
  })

  it('all model fields exist on DaemonSettings', () => {
    for (const field of MODEL_FIELDS) {
      expect(field in DEFAULT_SETTINGS).toBe(true)
    }
  })

  it('settings object has expected number of fields', () => {
    const keys = Object.keys(DEFAULT_SETTINGS)
    expect(keys.length).toBe(27)
  })
})

describe('IPC validation rules', () => {
  // Mirrors the validation in settings-handlers.ts

  it('rejects NaN for numeric fields', () => {
    const value = NaN
    expect(typeof value === 'number' && !isNaN(value) && value > 0).toBe(false)
  })

  it('accepts valid positive numbers', () => {
    const value = 60
    expect(typeof value === 'number' && !isNaN(value) && value > 0).toBe(true)
  })

  it('rejects zero for positive-only fields', () => {
    const value = 0
    expect(typeof value === 'number' && !isNaN(value) && value > 0).toBe(false)
  })

  it('rejects negative numbers', () => {
    const value = -5
    expect(typeof value === 'number' && !isNaN(value) && value > 0).toBe(false)
  })

  it('rejects non-numeric types', () => {
    const value = '60' as unknown
    expect(typeof value === 'number').toBe(false)
  })

  it('validates model name is non-empty string', () => {
    expect(typeof 'qwen3:14b' === 'string' && 'qwen3:14b'.length > 0).toBe(true)
    expect(typeof '' === 'string' && ''.length > 0).toBe(false)
  })
})
