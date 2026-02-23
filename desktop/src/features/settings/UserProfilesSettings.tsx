/**
 * UserProfilesSettings component
 *
 * Manage user profile documents that tell BoBe about the user.
 * List with enable/disable toggles + Monaco editor for content.
 */

import { useEffect, useState } from 'react'
import { motion } from 'framer-motion'
import Editor from '@monaco-editor/react'
import { Plus, User, Trash2, Check, X } from 'lucide-react'
import { cn } from '@/lib/cn'
import { configureMonaco } from '@/lib/monaco-setup'
import { useTheme } from '@/hooks/useTheme'
import { getUserProfilesClient } from '@/lib/browser-settings-client'
import type { UserProfile, UserProfileCreateRequest } from '@/types/api'

export function UserProfilesSettings() {
  const [profiles, setProfiles] = useState<UserProfile[]>([])
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
  const client = getUserProfilesClient()
  const selected = profiles.find((p) => p.id === selectedId) ?? null

  useEffect(() => {
    loadProfiles()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only
  }, [])

  async function loadProfiles() {
    setLoading(true)
    setError(null)
    try {
      const res = await client.list()
      setProfiles(res.profiles)
      if (res.profiles.length > 0 && !selectedId) {
        selectProfile(res.profiles[0]!)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load profiles')
    } finally {
      setLoading(false)
    }
  }

  function selectProfile(profile: UserProfile) {
    if (dirty && !confirm('You have unsaved changes. Discard?')) return
    setSelectedId(profile.id)
    setEditorContent(profile.content)
    setDirty(false)
    setDeleteConfirm(null)
  }

  async function handleToggle(profile: UserProfile) {
    try {
      if (profile.enabled) {
        await client.disable(profile.id)
      } else {
        await client.enable(profile.id)
      }
      setProfiles((prev) =>
        prev.map((p) => (p.id === profile.id ? { ...p, enabled: !p.enabled } : p)),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to toggle profile')
    }
  }

  async function handleSave() {
    if (!selected) return
    setSaving(true)
    setError(null)
    try {
      const updated = await client.update(selected.id, { content: editorContent })
      setProfiles((prev) => prev.map((p) => (p.id === updated.id ? updated : p)))
      setDirty(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save profile')
    } finally {
      setSaving(false)
    }
  }

  async function handleCreate() {
    if (!newName.trim()) return
    setError(null)
    try {
      const data: UserProfileCreateRequest = {
        name: newName.trim().toLowerCase().replace(/\s+/g, '-'),
        content: `# ${newName.trim()}\n\nDescribe this user profile here...`,
      }
      const profile = await client.create(data)
      setProfiles((prev) => [...prev, profile])
      selectProfile(profile)
      setCreating(false)
      setNewName('')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create profile')
    }
  }

  async function handleDelete(id: string) {
    setError(null)
    try {
      await client.delete(id)
      setProfiles((prev) => prev.filter((p) => p.id !== id))
      if (selectedId === id) {
        setSelectedId(null)
        setEditorContent('')
        setDirty(false)
      }
      setDeleteConfirm(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete profile')
    }
  }

  if (loading) {
    return (
      <div className="settings-editor-loading">
        <div className="settings-editor-loading-spinner" />
        <span>Loading user profiles...</span>
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
                placeholder="profile-name"
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
              <Plus size={14} /> New Profile
            </button>
          )}
        </div>

        {profiles.length === 0 ? (
          <div className="settings-empty-state">
            <User size={32} className="settings-empty-icon" />
            <p className="settings-empty-text">No profiles yet</p>
            <p className="settings-empty-hint">
              Profiles tell BoBe about your background and preferences
            </p>
          </div>
        ) : (
          <ul className="settings-item-list">
            {profiles.map((profile) => (
              <li key={profile.id}>
                <div
                  role="button"
                  tabIndex={0}
                  className={cn(
                    'settings-list-item',
                    selectedId === profile.id && 'settings-list-item-selected',
                  )}
                  onClick={() => selectProfile(profile)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault()
                      selectProfile(profile)
                    }
                  }}
                >
                  <div className="settings-list-item-info">
                    <div className="settings-list-item-main">
                      <span className="settings-list-item-name">{profile.name}</span>
                      {profile.is_default && (
                        <span className="settings-list-item-badge">default</span>
                      )}
                    </div>
                    <span className="settings-list-item-meta">
                      {profile.content.slice(0, 60).replace(/\n/g, ' ')}...
                    </span>
                  </div>
                  <div className="settings-list-item-actions">
                    <button
                      type="button"
                      role="switch"
                      aria-checked={profile.enabled}
                      onClick={(e) => {
                        e.stopPropagation()
                        handleToggle(profile)
                      }}
                      className={cn(
                        'settings-toggle',
                        profile.enabled ? 'settings-toggle-on' : 'settings-toggle-off',
                      )}
                    >
                      <motion.span
                        className="settings-toggle-thumb"
                        animate={{ x: profile.enabled ? 18 : 2 }}
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
            <User size={32} className="settings-editor-empty-icon" />
            <p className="settings-editor-empty-text">Select a profile to edit</p>
          </div>
        )}
      </div>
    </>
  )
}
