/**
 * GoalsSettings component
 *
 * Manage BoBe's goals with priority, status, enable/disable.
 * List panel + editor with content textarea and controls.
 */

import { useEffect, useState } from 'react'
import { motion } from 'framer-motion'
import Editor from '@monaco-editor/react'
import { Plus, Target, Trash2, Check, Archive } from 'lucide-react'
import { cn } from '@/lib/cn'
import { configureMonaco } from '@/lib/monaco-setup'
import { useTheme } from '@/hooks/useTheme'
import { getGoalsClient } from '@/lib/browser-settings-client'
import type { Goal, GoalCreateRequest, GoalPriority } from '@/types/api'

const PRIORITY_COLORS: Record<GoalPriority, string> = {
  high: '#C67B5C',
  medium: '#D4A574',
  low: '#8B9A7D',
}

export function GoalsSettings() {
  const [goals, setGoals] = useState<Goal[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [editorContent, setEditorContent] = useState('')
  const [editPriority, setEditPriority] = useState<GoalPriority>('medium')
  const [creating, setCreating] = useState(false)
  const [newContent, setNewContent] = useState('')
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null)

  const { theme } = useTheme()
  const monacoTheme = theme.isDark ? 'vs-dark' : 'vs-light'
  const client = getGoalsClient()
  const selected = goals.find((g) => g.id === selectedId) ?? null

  useEffect(() => {
    loadGoals()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only
  }, [])

  async function loadGoals() {
    setLoading(true)
    setError(null)
    try {
      const res = await client.list()
      setGoals(res.goals)
      if (res.goals.length > 0 && !selectedId) {
        selectGoal(res.goals[0]!)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load goals')
    } finally {
      setLoading(false)
    }
  }

  function selectGoal(goal: Goal) {
    if (dirty && !confirm('You have unsaved changes. Discard?')) return
    setSelectedId(goal.id)
    setEditorContent(goal.content)
    setEditPriority(goal.priority)
    setDirty(false)
    setDeleteConfirm(null)
  }

  async function handleToggle(goal: Goal) {
    try {
      await client.update(goal.id, { enabled: !goal.enabled })
      setGoals((prev) => prev.map((g) => (g.id === goal.id ? { ...g, enabled: !g.enabled } : g)))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to toggle goal')
    }
  }

  async function handleSave() {
    if (!selected) return
    setSaving(true)
    setError(null)
    try {
      const updated = await client.update(selected.id, {
        content: editorContent,
        priority: editPriority,
      })
      setGoals((prev) => prev.map((g) => (g.id === updated.id ? updated : g)))
      setDirty(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save goal')
    } finally {
      setSaving(false)
    }
  }

  async function handleCreate() {
    if (!newContent.trim()) return
    setError(null)
    try {
      const data: GoalCreateRequest = {
        content: newContent.trim(),
        priority: 'medium',
      }
      const goal = await client.create(data)
      setGoals((prev) => [...prev, goal])
      selectGoal(goal)
      setCreating(false)
      setNewContent('')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create goal')
    }
  }

  async function handleDelete(id: string) {
    setError(null)
    try {
      await client.delete(id)
      setGoals((prev) => prev.filter((g) => g.id !== id))
      if (selectedId === id) {
        setSelectedId(null)
        setEditorContent('')
        setDirty(false)
      }
      setDeleteConfirm(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete goal')
    }
  }

  async function handleComplete(id: string) {
    setError(null)
    try {
      await client.complete(id)
      setGoals((prev) =>
        prev.map((g) => (g.id === id ? { ...g, status: 'completed' as const } : g)),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to complete goal')
    }
  }

  async function handleArchive(id: string) {
    setError(null)
    try {
      await client.archive(id)
      setGoals((prev) => prev.map((g) => (g.id === id ? { ...g, status: 'archived' as const } : g)))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to archive goal')
    }
  }

  if (loading) {
    return (
      <div className="settings-editor-loading">
        <div className="settings-editor-loading-spinner" />
        <span>Loading goals...</span>
      </div>
    )
  }

  return (
    <>
      {error && <div className="settings-error">{error}</div>}

      {/* List panel */}
      <div className="settings-list-panel">
        <div style={{ padding: '12px 16px', borderBottom: '1px solid var(--color-bobe-sand)' }}>
          {creating ? (
            <div
              className="settings-create-form"
              style={{ flexDirection: 'column', gap: '8px', alignItems: 'stretch' }}
            >
              <input
                className="settings-create-input"
                style={{ width: '100%' }}
                placeholder="What's the goal?"
                value={newContent}
                onChange={(e) => setNewContent(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
                autoFocus
              />
              <div style={{ display: 'flex', gap: '8px' }}>
                <button className="settings-button settings-button-primary" onClick={handleCreate}>
                  Create
                </button>
                <button
                  className="settings-button settings-button-secondary"
                  onClick={() => {
                    setCreating(false)
                    setNewContent('')
                  }}
                >
                  Cancel
                </button>
              </div>
            </div>
          ) : (
            <button
              className="settings-button settings-button-primary settings-button-icon"
              onClick={() => setCreating(true)}
            >
              <Plus size={14} /> New Goal
            </button>
          )}
        </div>

        {goals.length === 0 ? (
          <div className="settings-empty-state">
            <Target size={32} className="settings-empty-icon" />
            <p className="settings-empty-text">No goals yet</p>
            <p className="settings-empty-hint">Goals help BoBe stay focused on what matters</p>
          </div>
        ) : (
          <ul className="settings-item-list">
            {goals.map((goal) => (
              <li key={goal.id}>
                <div
                  role="button"
                  tabIndex={0}
                  className={cn(
                    'settings-list-item',
                    selectedId === goal.id && 'settings-list-item-selected',
                  )}
                  onClick={() => selectGoal(goal)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault()
                      selectGoal(goal)
                    }
                  }}
                >
                  <div className="settings-list-item-info">
                    <div className="settings-list-item-main">
                      <span
                        style={{
                          width: 8,
                          height: 8,
                          borderRadius: '50%',
                          background: PRIORITY_COLORS[goal.priority],
                          flexShrink: 0,
                        }}
                      />
                      <span className="settings-list-item-name">{goal.content.slice(0, 40)}</span>
                    </div>
                    <span className="settings-list-item-meta">
                      {goal.status} &middot; {goal.priority} &middot; {goal.source}
                    </span>
                  </div>
                  <div className="settings-list-item-actions">
                    <button
                      type="button"
                      role="switch"
                      aria-checked={goal.enabled}
                      onClick={(e) => {
                        e.stopPropagation()
                        handleToggle(goal)
                      }}
                      className={cn(
                        'settings-toggle',
                        goal.enabled ? 'settings-toggle-on' : 'settings-toggle-off',
                      )}
                    >
                      <motion.span
                        className="settings-toggle-thumb"
                        animate={{ x: goal.enabled ? 18 : 2 }}
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

      {/* Editor panel */}
      <div className="settings-editor-panel">
        {selected ? (
          <div className="settings-editor">
            <div className="settings-editor-header">
              <div className="settings-editor-title">
                <span className="settings-editor-name">Goal</span>
                {dirty && <span className="settings-editor-dirty">unsaved</span>}
                <span
                  className="settings-list-item-badge"
                  style={{ background: PRIORITY_COLORS[editPriority], color: 'white' }}
                >
                  {editPriority}
                </span>
                <span className="settings-list-item-badge">{selected.status}</span>
              </div>
              <div style={{ display: 'flex', gap: '8px', marginTop: '8px' }}>
                <select
                  value={editPriority}
                  onChange={(e) => {
                    setEditPriority(e.target.value as GoalPriority)
                    setDirty(true)
                  }}
                  style={{
                    padding: '4px 8px',
                    borderRadius: '6px',
                    border: '1px solid var(--color-bobe-sand)',
                    background: 'var(--color-bobe-warm-white)',
                    fontSize: '12px',
                    color: 'var(--color-bobe-charcoal)',
                  }}
                >
                  <option value="high">High Priority</option>
                  <option value="medium">Medium Priority</option>
                  <option value="low">Low Priority</option>
                </select>
                {selected.status === 'active' && (
                  <>
                    <button
                      className="settings-button settings-button-secondary settings-button-icon"
                      onClick={() => handleComplete(selected.id)}
                    >
                      <Check size={14} /> Complete
                    </button>
                    <button
                      className="settings-button settings-button-secondary settings-button-icon"
                      onClick={() => handleArchive(selected.id)}
                    >
                      <Archive size={14} /> Archive
                    </button>
                  </>
                )}
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
                  setDirty(val !== selected.content || editPriority !== selected.priority)
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
                      setEditPriority(selected.priority)
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
            <Target size={32} className="settings-editor-empty-icon" />
            <p className="settings-editor-empty-text">Select a goal to edit</p>
          </div>
        )}
      </div>
    </>
  )
}
