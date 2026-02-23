/**
 * MemoriesSettings component
 *
 * Browse and manage BoBe's memories.
 * List panel with filters + Monaco editor for markdown content.
 */

import { useEffect, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import Editor from '@monaco-editor/react'
import { Plus, Brain, Trash2, AlertCircle } from 'lucide-react'
import { cn } from '@/lib/cn'
import { configureMonaco } from '@/lib/monaco-setup'
import { useTheme } from '@/hooks/useTheme'
import { getMemoriesClient } from '@/lib/browser-settings-client'
import type { Memory, MemoryCreateRequest, MemoryCategory, MemoryType } from '@/types/api'

const CATEGORY_LABELS: Record<MemoryCategory, string> = {
  preference: 'Pref',
  pattern: 'Pattern',
  fact: 'Fact',
  interest: 'Interest',
  general: 'General',
  observation: 'Visual',
}

const TYPE_LABELS: Record<MemoryType, string> = {
  short_term: 'Short',
  long_term: 'Long',
  explicit: 'Explicit',
}

const selectStyle: React.CSSProperties = {
  padding: '3px 6px',
  borderRadius: '5px',
  border: '1px solid var(--color-bobe-sand)',
  background: 'var(--color-bobe-warm-white)',
  fontSize: '11px',
  color: 'var(--color-bobe-charcoal)',
}

export function MemoriesSettings() {
  const [memories, setMemories] = useState<Memory[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [editorContent, setEditorContent] = useState('')
  const [editCategory, setEditCategory] = useState<MemoryCategory>('general')
  const [creating, setCreating] = useState(false)
  const [newContent, setNewContent] = useState('')
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null)
  const [filterCategory, setFilterCategory] = useState<MemoryCategory | ''>('')
  const [filterType, setFilterType] = useState<MemoryType | ''>('')

  const { theme } = useTheme()
  const monacoTheme = theme.isDark ? 'vs-dark' : 'vs-light'
  const client = getMemoriesClient()
  const selected = memories.find((m) => m.id === selectedId) ?? null

  useEffect(() => {
    loadMemories()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- re-fetch on filter change only
  }, [filterCategory, filterType])

  async function loadMemories() {
    setLoading(true)
    setError(null)
    try {
      const params: Record<string, unknown> = { limit: 200 }
      if (filterCategory) params.category = filterCategory
      if (filterType) params.memory_type = filterType
      const res = await client.list(params as Parameters<typeof client.list>[0])
      setMemories(res.memories)
      if (res.memories.length > 0 && !selectedId) {
        selectMemory(res.memories[0]!)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load memories')
    } finally {
      setLoading(false)
    }
  }

  function selectMemory(memory: Memory) {
    if (dirty && !confirm('You have unsaved changes. Discard?')) return
    setSelectedId(memory.id)
    setEditorContent(memory.content)
    setEditCategory(memory.category)
    setDirty(false)
    setDeleteConfirm(null)
  }

  async function handleToggle(memory: Memory) {
    try {
      if (memory.enabled) {
        await client.disable(memory.id)
      } else {
        await client.enable(memory.id)
      }
      setMemories((prev) =>
        prev.map((m) => (m.id === memory.id ? { ...m, enabled: !m.enabled } : m)),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to toggle memory')
    }
  }

  async function handleSave() {
    if (!selected) return
    setSaving(true)
    setError(null)
    try {
      const updated = await client.update(selected.id, {
        content: editorContent,
        category: editCategory,
      })
      setMemories((prev) => prev.map((m) => (m.id === updated.id ? updated : m)))
      setDirty(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save memory')
    } finally {
      setSaving(false)
    }
  }

  async function handleCreate() {
    if (!newContent.trim()) return
    setError(null)
    try {
      const data: MemoryCreateRequest = {
        content: newContent.trim(),
        category: 'general',
        memory_type: 'explicit',
      }
      const memory = await client.create(data)
      setMemories((prev) => [memory, ...prev])
      selectMemory(memory)
      setCreating(false)
      setNewContent('')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create memory')
    }
  }

  async function handleDelete(id: string) {
    setError(null)
    try {
      await client.delete(id)
      setMemories((prev) => prev.filter((m) => m.id !== id))
      if (selectedId === id) {
        setSelectedId(null)
        setEditorContent('')
        setDirty(false)
      }
      setDeleteConfirm(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete memory')
    }
  }

  return (
    <>
      {/* List panel */}
      <div className="settings-list-panel" style={{ display: 'flex', flexDirection: 'column' }}>
        {/* Filters row */}
        <div
          style={{
            padding: '8px 12px',
            borderBottom: '1px solid var(--color-bobe-sand)',
            display: 'flex',
            gap: '6px',
            alignItems: 'center',
          }}
        >
          <select
            value={filterCategory}
            onChange={(e) => setFilterCategory(e.target.value as MemoryCategory | '')}
            style={selectStyle}
          >
            <option value="">Category</option>
            {Object.entries(CATEGORY_LABELS).map(([k, v]) => (
              <option key={k} value={k}>
                {v}
              </option>
            ))}
          </select>
          <select
            value={filterType}
            onChange={(e) => setFilterType(e.target.value as MemoryType | '')}
            style={selectStyle}
          >
            <option value="">Type</option>
            {Object.entries(TYPE_LABELS).map(([k, v]) => (
              <option key={k} value={k}>
                {v}
              </option>
            ))}
          </select>
          <span style={{ fontSize: '10px', color: 'var(--color-bobe-clay)', marginLeft: 'auto' }}>
            {memories.length}
          </span>
          <button
            className="settings-button settings-button-primary"
            style={{ padding: '3px 8px', fontSize: '11px' }}
            onClick={() => setCreating(true)}
          >
            <Plus size={12} />
          </button>
        </div>

        {/* Error */}
        {error && (
          <div
            className="settings-error"
            style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '12px' }}
          >
            <AlertCircle size={13} />
            <span style={{ flex: 1 }}>{error}</span>
            <button
              className="settings-button settings-button-secondary"
              style={{ padding: '2px 6px', fontSize: '10px' }}
              onClick={loadMemories}
            >
              Retry
            </button>
          </div>
        )}

        {/* Create inline */}
        <AnimatePresence>
          {creating && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              style={{ overflow: 'hidden', borderBottom: '1px solid var(--color-bobe-sand)' }}
            >
              <div
                style={{
                  padding: '10px 12px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '6px',
                }}
              >
                <textarea
                  placeholder="What should BoBe remember?"
                  value={newContent}
                  onChange={(e) => setNewContent(e.target.value)}
                  style={{
                    width: '100%',
                    minHeight: '50px',
                    padding: '6px 8px',
                    borderRadius: '6px',
                    border: '1px solid var(--color-bobe-sand)',
                    background: 'var(--color-bobe-warm-white)',
                    fontSize: '12px',
                    color: 'var(--color-bobe-charcoal)',
                    resize: 'vertical',
                    outline: 'none',
                    fontFamily: 'inherit',
                  }}
                  autoFocus
                />
                <div style={{ display: 'flex', gap: '6px' }}>
                  <button
                    className="settings-button settings-button-primary"
                    style={{ fontSize: '12px' }}
                    onClick={handleCreate}
                  >
                    Create
                  </button>
                  <button
                    className="settings-button settings-button-secondary"
                    style={{ fontSize: '12px' }}
                    onClick={() => {
                      setCreating(false)
                      setNewContent('')
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* List */}
        <div style={{ flex: 1, overflowY: 'auto' }}>
          {loading ? (
            <div className="settings-editor-loading" style={{ padding: '40px 0' }}>
              <div className="settings-editor-loading-spinner" />
              <span>Loading...</span>
            </div>
          ) : memories.length === 0 ? (
            <div className="settings-empty-state">
              <Brain size={28} className="settings-empty-icon" />
              <p className="settings-empty-text">No memories</p>
            </div>
          ) : (
            <ul className="settings-item-list">
              {memories.map((memory) => (
                <li key={memory.id}>
                  <div
                    role="button"
                    tabIndex={0}
                    className={cn(
                      'settings-list-item',
                      selectedId === memory.id && 'settings-list-item-selected',
                    )}
                    onClick={() => selectMemory(memory)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault()
                        selectMemory(memory)
                      }
                    }}
                    style={{ opacity: memory.enabled ? 1 : 0.5 }}
                  >
                    <div className="settings-list-item-info">
                      <div className="settings-list-item-main">
                        <span className="settings-list-item-name">
                          {memory.content.slice(0, 45)}
                        </span>
                      </div>
                      <span className="settings-list-item-meta">
                        {CATEGORY_LABELS[memory.category]} &middot;{' '}
                        {TYPE_LABELS[memory.memory_type]}
                      </span>
                    </div>
                    <div className="settings-list-item-actions">
                      <button
                        type="button"
                        role="switch"
                        aria-checked={memory.enabled}
                        onClick={(e) => {
                          e.stopPropagation()
                          handleToggle(memory)
                        }}
                        className={cn(
                          'settings-toggle',
                          memory.enabled ? 'settings-toggle-on' : 'settings-toggle-off',
                        )}
                      >
                        <motion.span
                          className="settings-toggle-thumb"
                          animate={{ x: memory.enabled ? 18 : 2 }}
                          transition={{ type: 'spring', stiffness: 500, damping: 30 }}
                        />
                      </button>
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      {/* Editor panel */}
      <div className="settings-editor-panel">
        {selected ? (
          <div className="settings-editor">
            <div className="settings-editor-header">
              <div className="settings-editor-title">
                <span className="settings-editor-name">Memory</span>
                {dirty && <span className="settings-editor-dirty">unsaved</span>}
                <span className="settings-list-item-badge">{CATEGORY_LABELS[editCategory]}</span>
                <span className="settings-list-item-badge">
                  {TYPE_LABELS[selected.memory_type]}
                </span>
              </div>
              <div style={{ display: 'flex', gap: '8px', marginTop: '6px', alignItems: 'center' }}>
                <select
                  value={editCategory}
                  onChange={(e) => {
                    setEditCategory(e.target.value as MemoryCategory)
                    setDirty(true)
                  }}
                  style={selectStyle}
                >
                  {Object.entries(CATEGORY_LABELS).map(([k, v]) => (
                    <option key={k} value={k}>
                      {v}
                    </option>
                  ))}
                </select>
                <span
                  style={{ fontSize: '11px', color: 'var(--color-bobe-clay)', marginLeft: 'auto' }}
                >
                  {new Date(selected.created_at).toLocaleDateString()}
                </span>
              </div>
            </div>

            <div className="settings-editor-monaco">
              <Editor
                height="100%"
                language="markdown"
                theme={monacoTheme}
                beforeMount={configureMonaco}
                value={editorContent}
                onChange={(val) => {
                  setEditorContent(val ?? '')
                  setDirty(val !== selected.content || editCategory !== selected.category)
                }}
                options={{
                  minimap: { enabled: false },
                  lineNumbers: 'on',
                  wordWrap: 'on',
                  fontSize: 13,
                  fontFamily: "Menlo, Monaco, 'Courier New', monospace",
                  padding: { top: 8 },
                  scrollBeyondLastLine: false,
                  renderLineHighlight: 'line',
                  bracketPairColorization: { enabled: true },
                  guides: { bracketPairs: true, indentation: true },
                  folding: true,
                  glyphMargin: false,
                  smoothScrolling: true,
                  cursorBlinking: 'smooth',
                  cursorSmoothCaretAnimation: 'on',
                  scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10 },
                }}
              />
            </div>

            <div className="settings-editor-toolbar">
              <div className="settings-editor-toolbar-left">
                {deleteConfirm === selected.id ? (
                  <div className="settings-delete-confirm">
                    <span className="settings-delete-confirm-text">Delete?</span>
                    <button
                      className="settings-button settings-button-danger"
                      onClick={() => handleDelete(selected.id)}
                    >
                      Yes
                    </button>
                    <button
                      className="settings-button settings-button-secondary"
                      onClick={() => setDeleteConfirm(null)}
                    >
                      No
                    </button>
                  </div>
                ) : (
                  <button
                    className="settings-button settings-button-secondary settings-button-icon"
                    onClick={() => setDeleteConfirm(selected.id)}
                  >
                    <Trash2 size={14} /> Delete
                  </button>
                )}
              </div>
              <div className="settings-editor-toolbar-right">
                {dirty && (
                  <button
                    className="settings-button settings-button-secondary"
                    onClick={() => {
                      setEditorContent(selected.content)
                      setEditCategory(selected.category)
                      setDirty(false)
                    }}
                  >
                    Discard
                  </button>
                )}
                <button
                  className="settings-button settings-button-primary"
                  onClick={handleSave}
                  disabled={!dirty || saving}
                >
                  {saving ? 'Saving...' : 'Save'}
                </button>
              </div>
            </div>
          </div>
        ) : (
          <div className="settings-editor-empty">
            <Brain size={32} className="settings-editor-empty-icon" />
            <p className="settings-editor-empty-text">Select a memory to edit</p>
          </div>
        )}
      </div>
    </>
  )
}
