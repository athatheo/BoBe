/**
 * SoulsSettings component
 *
 * Manage BoBe's personality/soul documents.
 * List with enable/disable toggles + Monaco editor for content.
 */

import { useEffect, useState } from 'react'
import { motion } from 'framer-motion'
import Editor from '@monaco-editor/react'
import { Plus, Sparkles, Trash2, Check, X } from 'lucide-react'
import { cn } from '@/lib/cn'
import { configureMonaco } from '@/lib/monaco-setup'
import { useTheme } from '@/hooks/useTheme'
import { getSoulsClient } from '@/lib/browser-settings-client'
import type { Soul, SoulCreateRequest } from '@/types/api'

export function SoulsSettings() {
  const [souls, setSouls] = useState<Soul[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [editorContent, setEditorContent] = useState('')
  const [creating, setCreating] = useState(false)
  const [newName, setNewName] = useState('')
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null)

  const { theme } = useTheme()
  const monacoTheme = theme.isDark ? 'vs-dark' : 'vs-light'
  const client = getSoulsClient()
  const selected = souls.find((s) => s.id === selectedId) ?? null

  useEffect(() => {
    loadSouls()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only
  }, [])

  async function loadSouls() {
    setLoading(true)
    setError(null)
    console.log('[SoulsSettings] loadSouls starting, client:', client ? 'exists' : 'null')
    try {
      const res = await client.list()
      console.log('[SoulsSettings] response:', JSON.stringify(res).slice(0, 200))
      setSouls(res.souls)
      if (res.souls.length > 0 && !selectedId) {
        selectSoul(res.souls[0]!)
      }
    } catch (err) {
      console.error('[SoulsSettings] ERROR:', err)
      setError(err instanceof Error ? err.message : 'Failed to load souls')
    } finally {
      console.log('[SoulsSettings] loading complete')
      setLoading(false)
    }
  }

  function selectSoul(soul: Soul) {
    if (dirty && !confirm('You have unsaved changes. Discard?')) return
    setSelectedId(soul.id)
    setEditorContent(soul.content)
    setDirty(false)
    setDeleteConfirm(null)
  }

  async function handleToggle(soul: Soul) {
    try {
      if (soul.enabled) {
        await client.disable(soul.id)
      } else {
        await client.enable(soul.id)
      }
      setSouls((prev) => prev.map((s) => (s.id === soul.id ? { ...s, enabled: !s.enabled } : s)))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to toggle soul')
    }
  }

  async function handleSave() {
    if (!selected) return
    setSaving(true)
    setError(null)
    try {
      const updated = await client.update(selected.id, { content: editorContent })
      setSouls((prev) => prev.map((s) => (s.id === updated.id ? updated : s)))
      setDirty(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save soul')
    } finally {
      setSaving(false)
    }
  }

  async function handleCreate() {
    if (!newName.trim()) return
    setError(null)
    try {
      const data: SoulCreateRequest = {
        name: newName.trim().toLowerCase().replace(/\s+/g, '-'),
        content: `# ${newName.trim()}\n\nDescribe this soul's personality here...`,
      }
      const soul = await client.create(data)
      setSouls((prev) => [...prev, soul])
      selectSoul(soul)
      setCreating(false)
      setNewName('')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create soul')
    }
  }

  async function handleDelete(id: string) {
    setError(null)
    try {
      await client.delete(id)
      setSouls((prev) => prev.filter((s) => s.id !== id))
      if (selectedId === id) {
        setSelectedId(null)
        setEditorContent('')
        setDirty(false)
      }
      setDeleteConfirm(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete soul')
    }
  }

  if (loading) {
    return (
      <div className="settings-editor-loading">
        <div className="settings-editor-loading-spinner" />
        <span>Loading souls...</span>
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
            <div className="settings-create-form">
              <input
                className="settings-create-input"
                placeholder="soul-name"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
                autoFocus
              />
              <button className="settings-button settings-button-primary" onClick={handleCreate}>
                <Check size={14} />
              </button>
              <button
                className="settings-button settings-button-secondary"
                onClick={() => {
                  setCreating(false)
                  setNewName('')
                }}
              >
                <X size={14} />
              </button>
            </div>
          ) : (
            <button
              className="settings-button settings-button-primary settings-button-icon"
              onClick={() => setCreating(true)}
            >
              <Plus size={14} /> New Soul
            </button>
          )}
        </div>

        {souls.length === 0 ? (
          <div className="settings-empty-state">
            <Sparkles size={32} className="settings-empty-icon" />
            <p className="settings-empty-text">No souls yet</p>
            <p className="settings-empty-hint">Create one to define BoBe's personality</p>
          </div>
        ) : (
          <ul className="settings-item-list">
            {souls.map((soul) => (
              <li key={soul.id}>
                <div
                  role="button"
                  tabIndex={0}
                  className={cn(
                    'settings-list-item',
                    selectedId === soul.id && 'settings-list-item-selected',
                  )}
                  onClick={() => selectSoul(soul)}
                  onKeyDown={(e) => e.key === 'Enter' && selectSoul(soul)}
                >
                  <div className="settings-list-item-info">
                    <div className="settings-list-item-main">
                      <span className="settings-list-item-name">{soul.name}</span>
                      {soul.is_default && <span className="settings-list-item-badge">default</span>}
                    </div>
                    <span className="settings-list-item-meta">
                      {soul.content.slice(0, 60).replace(/\n/g, ' ')}...
                    </span>
                  </div>
                  <div className="settings-list-item-actions">
                    <button
                      type="button"
                      role="switch"
                      aria-checked={soul.enabled}
                      onClick={(e) => {
                        e.stopPropagation()
                        handleToggle(soul)
                      }}
                      className={cn(
                        'settings-toggle',
                        soul.enabled ? 'settings-toggle-on' : 'settings-toggle-off',
                      )}
                    >
                      <motion.span
                        className="settings-toggle-thumb"
                        animate={{ x: soul.enabled ? 18 : 2 }}
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
                <span className="settings-editor-name">{selected.name}</span>
                {dirty && <span className="settings-editor-dirty">unsaved</span>}
                {selected.is_default && <span className="settings-list-item-badge">default</span>}
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
                  setDirty(val !== selected.content)
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
                  !selected.is_default && (
                    <button
                      className="settings-button settings-button-secondary settings-button-icon"
                      onClick={() => setDeleteConfirm(selected.id)}
                    >
                      <Trash2 size={14} /> Delete
                    </button>
                  )
                )}
              </div>
              <div className="settings-editor-toolbar-right">
                {dirty && (
                  <button
                    className="settings-button settings-button-secondary"
                    onClick={() => {
                      setEditorContent(selected.content)
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
            <Sparkles size={32} className="settings-editor-empty-icon" />
            <p className="settings-editor-empty-text">Select a soul to edit</p>
          </div>
        )}
      </div>
    </>
  )
}
