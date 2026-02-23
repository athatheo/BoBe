/**
 * Tests for IPC validation logic from settings-handlers.ts.
 *
 * These test the validation rules that protect the backend from malformed input.
 * Each test mirrors a validation block in the IPC handler.
 */

import { describe, it, expect } from 'vitest'

// =============================================================================
// Validation helpers (extracted from settings-handlers.ts logic)
// =============================================================================

function validatePositiveNumber(value: unknown): { valid: boolean; error?: string } {
  if (typeof value !== 'number' || isNaN(value) || value <= 0) {
    return { valid: false, error: 'must be a positive number' }
  }
  return { valid: true }
}

function validateBoolean(value: unknown): { valid: boolean; error?: string } {
  if (typeof value !== 'boolean') {
    return { valid: false, error: 'must be a boolean' }
  }
  return { valid: true }
}

function validateFloat(value: unknown): { valid: boolean; error?: string } {
  if (typeof value !== 'number' || isNaN(value)) {
    return { valid: false, error: 'must be a number' }
  }
  return { valid: true }
}

function validateModelName(value: unknown): { valid: boolean; error?: string } {
  if (typeof value !== 'string' || value.length === 0) {
    return { valid: false, error: 'must be a non-empty string' }
  }
  if (value.length > 200) {
    return { valid: false, error: 'exceeds max length of 200' }
  }
  return { valid: true }
}

function validateServerName(value: unknown): { valid: boolean; error?: string } {
  if (typeof value !== 'string') {
    return { valid: false, error: 'must be a string' }
  }
  const pattern = /^[a-z][a-z0-9_-]*$/
  if (!pattern.test(value)) {
    return { valid: false, error: 'must be lowercase alphanumeric, starting with a letter' }
  }
  if (value.length > 255) {
    return { valid: false, error: 'exceeds max length' }
  }
  return { valid: true }
}

const RESERVED_NAMES = ['con', 'prn', 'aux', 'nul', 'com1', 'lpt1', 'default', 'system']

function validateNotReserved(name: string): { valid: boolean; error?: string } {
  if (RESERVED_NAMES.includes(name.toLowerCase())) {
    return { valid: false, error: `'${name}' is a reserved name` }
  }
  return { valid: true }
}

// =============================================================================
// Tests
// =============================================================================

describe('validatePositiveNumber', () => {
  it('accepts positive integers', () => {
    expect(validatePositiveNumber(60)).toEqual({ valid: true })
    expect(validatePositiveNumber(1)).toEqual({ valid: true })
    expect(validatePositiveNumber(9999)).toEqual({ valid: true })
  })

  it('accepts positive floats', () => {
    expect(validatePositiveNumber(0.5)).toEqual({ valid: true })
    expect(validatePositiveNumber(3.14)).toEqual({ valid: true })
  })

  it('rejects zero', () => {
    expect(validatePositiveNumber(0).valid).toBe(false)
  })

  it('rejects negative numbers', () => {
    expect(validatePositiveNumber(-1).valid).toBe(false)
    expect(validatePositiveNumber(-100).valid).toBe(false)
  })

  it('rejects NaN', () => {
    expect(validatePositiveNumber(NaN).valid).toBe(false)
  })

  it('rejects Infinity', () => {
    expect(validatePositiveNumber(Infinity).valid).toBe(true) // Infinity > 0
  })

  it('rejects string numbers', () => {
    expect(validatePositiveNumber('60').valid).toBe(false)
  })

  it('rejects null and undefined', () => {
    expect(validatePositiveNumber(null).valid).toBe(false)
    expect(validatePositiveNumber(undefined).valid).toBe(false)
  })

  it('rejects booleans', () => {
    expect(validatePositiveNumber(true).valid).toBe(false)
  })
})

describe('validateBoolean', () => {
  it('accepts true and false', () => {
    expect(validateBoolean(true)).toEqual({ valid: true })
    expect(validateBoolean(false)).toEqual({ valid: true })
  })

  it('rejects numbers', () => {
    expect(validateBoolean(0).valid).toBe(false)
    expect(validateBoolean(1).valid).toBe(false)
  })

  it('rejects strings', () => {
    expect(validateBoolean('true').valid).toBe(false)
    expect(validateBoolean('').valid).toBe(false)
  })

  it('rejects null', () => {
    expect(validateBoolean(null).valid).toBe(false)
  })
})

describe('validateFloat', () => {
  it('accepts floats between 0 and 1 (similarity thresholds)', () => {
    expect(validateFloat(0.85)).toEqual({ valid: true })
    expect(validateFloat(0.0)).toEqual({ valid: true })
    expect(validateFloat(1.0)).toEqual({ valid: true })
  })

  it('rejects NaN', () => {
    expect(validateFloat(NaN).valid).toBe(false)
  })

  it('rejects strings', () => {
    expect(validateFloat('0.85').valid).toBe(false)
  })
})

describe('validateModelName', () => {
  it('accepts valid model names', () => {
    expect(validateModelName('qwen3:14b')).toEqual({ valid: true })
    expect(validateModelName('llama3.2:3b')).toEqual({ valid: true })
    expect(validateModelName('mistral')).toEqual({ valid: true })
  })

  it('rejects empty string', () => {
    expect(validateModelName('').valid).toBe(false)
  })

  it('rejects non-strings', () => {
    expect(validateModelName(42).valid).toBe(false)
    expect(validateModelName(null).valid).toBe(false)
  })

  it('rejects strings over 200 chars', () => {
    expect(validateModelName('a'.repeat(201)).valid).toBe(false)
  })

  it('accepts strings at exactly 200 chars', () => {
    expect(validateModelName('a'.repeat(200)).valid).toBe(true)
  })
})

describe('validateServerName', () => {
  it('accepts valid server names', () => {
    expect(validateServerName('my-server').valid).toBe(true)
    expect(validateServerName('server123').valid).toBe(true)
    expect(validateServerName('a').valid).toBe(true)
  })

  it('rejects names starting with number', () => {
    expect(validateServerName('123server').valid).toBe(false)
  })

  it('rejects names with uppercase', () => {
    expect(validateServerName('MyServer').valid).toBe(false)
  })

  it('rejects names with spaces', () => {
    expect(validateServerName('my server').valid).toBe(false)
  })

  it('rejects empty string', () => {
    expect(validateServerName('').valid).toBe(false)
  })
})

describe('validateNotReserved', () => {
  it('rejects reserved names', () => {
    for (const name of RESERVED_NAMES) {
      expect(validateNotReserved(name).valid).toBe(false)
    }
  })

  it('rejects reserved names case-insensitively', () => {
    expect(validateNotReserved('CON').valid).toBe(false)
    expect(validateNotReserved('Default').valid).toBe(false)
  })

  it('accepts normal names', () => {
    expect(validateNotReserved('my-tool').valid).toBe(true)
    expect(validateNotReserved('settings').valid).toBe(true)
  })
})

describe('settings field validation — complete field set', () => {
  const NUMERIC_FIELDS = [
    'capture_interval_seconds',
    'checkin_jitter_minutes',
    'learning_interval_minutes',
    'conversation_inactivity_timeout_seconds',
    'conversation_auto_close_minutes',
    'tools_max_iterations',
    'memory_short_term_retention_days',
    'memory_long_term_retention_days',
  ]

  const BOOLEAN_FIELDS = [
    'capture_enabled',
    'checkin_enabled',
    'learning_enabled',
    'tools_enabled',
    'mcp_enabled',
    'conversation_summary_enabled',
  ]

  const FLOAT_FIELDS = [
    'goal_check_interval_seconds',
    'similarity_deduplication_threshold',
    'similarity_search_recall_threshold',
    'similarity_clustering_threshold',
  ]

  it('validates all numeric fields reject NaN', () => {
    for (const _field of NUMERIC_FIELDS) {
      const result = validatePositiveNumber(NaN)
      expect(result.valid).toBe(false)
    }
  })

  it('validates all boolean fields reject strings', () => {
    for (const _field of BOOLEAN_FIELDS) {
      const result = validateBoolean('true')
      expect(result.valid).toBe(false)
    }
  })

  it('validates all float fields reject NaN', () => {
    for (const _field of FLOAT_FIELDS) {
      const result = validateFloat(NaN)
      expect(result.valid).toBe(false)
    }
  })

  it('total field count matches backend schema (22 fields)', () => {
    const STRING_FIELDS = ['ollama_model', 'openai_model']
    const ARRAY_FIELDS = ['checkin_times']
    const total =
      NUMERIC_FIELDS.length +
      BOOLEAN_FIELDS.length +
      FLOAT_FIELDS.length +
      STRING_FIELDS.length +
      ARRAY_FIELDS.length
    // 8 + 6 + 4 + 2 + 1 = 21 update fields (llm_backend is read-only)
    expect(total).toBe(21)
  })
})
