/**
 * MCPServersSettings component
 *
 * Manage MCP server connections.
 * List with connection status + form for adding servers + JSON config editor.
 */

import { useEffect, useState } from 'react'
import Editor from '@monaco-editor/react'
import { Plus, Server, Trash2, X, RefreshCw, Wifi, WifiOff, AlertCircle, Ban } from 'lucide-react'
import { cn } from '@/lib/cn'
import { configureMonaco } from '@/lib/monaco-setup'
import { useTheme } from '@/hooks/useTheme'
import { getMCPServersClient } from '@/lib/browser-settings-client'
import type { MCPServer, MCPServerCreateRequest } from '@/types/api'

export function MCPServersSettings() {
  const [servers, setServers] = useState<MCPServer[]>([])
  const [selectedName, setSelectedName] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [creating, setCreating] = useState(false)
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null)
  const [reconnecting, setReconnecting] = useState<string | null>(null)

  // Create form
  const [newName, setNewName] = useState('')
  const [newCommand, setNewCommand] = useState('')
  const [newArgs, setNewArgs] = useState('')
  const [newEnvJson, setNewEnvJson] = useState('{}')
  const [newExcludedTools, setNewExcludedTools] = useState('')

  // Excluded tools editing
  const [excludedInput, setExcludedInput] = useState('')
  const [updatingExcluded, setUpdatingExcluded] = useState(false)

  const { theme } = useTheme()
  const monacoTheme = theme.isDark ? 'vs-dark' : 'vs-light'
  const client = getMCPServersClient()
  const selected = servers.find((s) => s.name === selectedName) ?? null

  useEffect(() => {
    loadServers()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only
  }, [])

  async function loadServers() {
    setLoading(true)
    setError(null)
    try {
      const [listRes, configsRes] = await Promise.all([
        client.list(),
        client.listConfigs().catch(() => null),
      ])
      // Build a lookup from server_name → config (id + excluded_tools)
      const configMap = new Map((configsRes?.configs ?? []).map((c) => [c.server_name, c]))
      const merged: MCPServer[] = listRes.servers.map((s) => {
        const cfg = configMap.get(s.name)
        return {
          ...s,
          id: cfg?.id ?? '',
          excluded_tools: cfg?.excluded_tools ?? [],
        }
      })
      setServers(merged)
      if (merged.length > 0 && !selectedName) {
        setSelectedName(merged[0]!.name)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load MCP servers')
    } finally {
      setLoading(false)
    }
  }

  async function handleCreate() {
    if (!newName.trim() || !newCommand.trim()) return
    setError(null)
    try {
      let env: Record<string, string> = {}
      try {
        env = JSON.parse(newEnvJson)
      } catch {
        setError('Invalid JSON for environment variables')
        return
      }
      const excludedTools = newExcludedTools.trim()
        ? newExcludedTools
            .split(',')
            .map((t) => t.trim())
            .filter(Boolean)
        : []
      const data: MCPServerCreateRequest = {
        name: newName.trim(),
        command: newCommand.trim(),
        args: newArgs.trim() ? newArgs.trim().split(/\s+/) : [],
        env,
        ...(excludedTools.length > 0 && { excluded_tools: excludedTools }),
      }
      const result = await client.create(data)
      await loadServers()
      setSelectedName(result.name)
      setCreating(false)
      setNewName('')
      setNewCommand('')
      setNewArgs('')
      setNewEnvJson('{}')
      setNewExcludedTools('')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create MCP server')
    }
  }

  async function handleDelete(name: string) {
    setError(null)
    try {
      await client.delete(name)
      setServers((prev) => prev.filter((s) => s.name !== name))
      if (selectedName === name) {
        setSelectedName(null)
      }
      setDeleteConfirm(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete MCP server')
    }
  }

  async function handleReconnect(name: string) {
    setReconnecting(name)
    setError(null)
    try {
      const result = await client.reconnect(name)
      setServers((prev) =>
        prev.map((s) =>
          s.name === name
            ? {
                ...s,
                connected: result.connected,
                tool_count: result.tool_count,
                error: result.error,
              }
            : s,
        ),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reconnect')
    } finally {
      setReconnecting(null)
    }
  }

  async function handleAddExcludedTool(serverId: string, toolName: string) {
    const server = servers.find((s) => s.id === serverId)
    if (!server || !toolName.trim()) return
    const updated = [...(server.excluded_tools ?? []), toolName.trim()]
    await handleUpdateExcludedTools(serverId, updated)
  }

  async function handleRemoveExcludedTool(serverId: string, toolName: string) {
    const server = servers.find((s) => s.id === serverId)
    if (!server) return
    const updated = (server.excluded_tools ?? []).filter((t) => t !== toolName)
    await handleUpdateExcludedTools(serverId, updated)
  }

  async function handleUpdateExcludedTools(serverId: string, excludedTools: string[]) {
    setUpdatingExcluded(true)
    setError(null)
    try {
      await client.update(serverId, { excluded_tools: excludedTools })
      setServers((prev) =>
        prev.map((s) => (s.id === serverId ? { ...s, excluded_tools: excludedTools } : s)),
      )
      setExcludedInput('')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update excluded tools')
    } finally {
      setUpdatingExcluded(false)
    }
  }

  const inputStyle: React.CSSProperties = {
    padding: '6px 10px',
    borderRadius: '6px',
    border: '1px solid var(--color-bobe-sand)',
    background: 'var(--color-bobe-warm-white)',
    fontSize: '13px',
    color: 'var(--color-bobe-charcoal)',
    width: '100%',
    outline: 'none',
  }

  if (loading) {
    return (
      <div className="settings-editor-loading">
        <div className="settings-editor-loading-spinner" />
        <span>Loading MCP servers...</span>
      </div>
    )
  }

  return (
    <>
      {error && <div className="settings-error">{error}</div>}

      {/* List panel */}
      <div className="settings-list-panel">
        <div style={{ padding: '12px 16px', borderBottom: '1px solid var(--color-bobe-sand)' }}>
          <button
            className="settings-button settings-button-primary settings-button-icon"
            onClick={() => setCreating(true)}
          >
            <Plus size={14} /> Add Server
          </button>
        </div>

        {servers.length === 0 && !creating ? (
          <div className="settings-empty-state">
            <Server size={32} className="settings-empty-icon" />
            <p className="settings-empty-text">No MCP servers</p>
            <p className="settings-empty-hint">Add servers to extend BoBe's capabilities</p>
          </div>
        ) : (
          <ul className="settings-item-list">
            {servers.map((server) => (
              <li key={server.name}>
                <button
                  className={cn(
                    'settings-list-item',
                    selectedName === server.name && 'settings-list-item-selected',
                  )}
                  onClick={() => {
                    setSelectedName(server.name)
                    setCreating(false)
                  }}
                >
                  <div className="settings-list-item-info">
                    <div className="settings-list-item-main">
                      {server.connected ? (
                        <Wifi
                          size={14}
                          style={{ color: 'var(--color-bobe-olive)', flexShrink: 0 }}
                        />
                      ) : (
                        <WifiOff
                          size={14}
                          style={{ color: 'var(--color-bobe-terracotta)', flexShrink: 0 }}
                        />
                      )}
                      <span className="settings-list-item-name">{server.name}</span>
                    </div>
                    <span className="settings-list-item-meta">
                      {server.command} &middot; {server.tool_count} tools
                      {server.error && ` &middot; Error`}
                    </span>
                  </div>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Right panel */}
      <div className="settings-editor-panel">
        {creating ? (
          <div className="settings-editor">
            <div className="settings-editor-header">
              <div className="settings-editor-title">
                <span className="settings-editor-name">New MCP Server</span>
              </div>
            </div>

            <div style={{ flex: 1, overflow: 'auto', padding: '16px' }}>
              <div
                style={{ display: 'flex', flexDirection: 'column', gap: '16px', maxWidth: '500px' }}
              >
                <div>
                  <label
                    style={{
                      fontSize: '13px',
                      fontWeight: 500,
                      color: 'var(--color-bobe-charcoal)',
                      display: 'block',
                      marginBottom: '4px',
                    }}
                  >
                    Server Name
                  </label>
                  <input
                    style={inputStyle}
                    placeholder="e.g. filesystem"
                    value={newName}
                    onChange={(e) => setNewName(e.target.value)}
                    autoFocus
                  />
                </div>
                <div>
                  <label
                    style={{
                      fontSize: '13px',
                      fontWeight: 500,
                      color: 'var(--color-bobe-charcoal)',
                      display: 'block',
                      marginBottom: '4px',
                    }}
                  >
                    Command
                  </label>
                  <input
                    style={inputStyle}
                    placeholder="e.g. npx or /usr/local/bin/mcp-server"
                    value={newCommand}
                    onChange={(e) => setNewCommand(e.target.value)}
                  />
                </div>
                <div>
                  <label
                    style={{
                      fontSize: '13px',
                      fontWeight: 500,
                      color: 'var(--color-bobe-charcoal)',
                      display: 'block',
                      marginBottom: '4px',
                    }}
                  >
                    Arguments (space-separated)
                  </label>
                  <input
                    style={inputStyle}
                    placeholder="e.g. -y @modelcontextprotocol/server-filesystem /tmp"
                    value={newArgs}
                    onChange={(e) => setNewArgs(e.target.value)}
                  />
                </div>
                <div>
                  <label
                    style={{
                      fontSize: '13px',
                      fontWeight: 500,
                      color: 'var(--color-bobe-charcoal)',
                      display: 'block',
                      marginBottom: '4px',
                    }}
                  >
                    Environment Variables (JSON)
                  </label>
                  <div
                    style={{
                      height: '180px',
                      border: '1px solid var(--color-bobe-sand)',
                      borderRadius: '6px',
                      overflow: 'hidden',
                    }}
                  >
                    <Editor
                      height="100%"
                      language="json"
                      theme={monacoTheme}
                      beforeMount={configureMonaco}
                      value={newEnvJson}
                      onChange={(val) => setNewEnvJson(val ?? '{}')}
                      options={{
                        minimap: { enabled: false },
                        lineNumbers: 'on',
                        wordWrap: 'on',
                        fontSize: 13,
                        fontFamily: "Menlo, Monaco, 'Courier New', monospace",
                        scrollBeyondLastLine: false,
                        renderLineHighlight: 'line',
                        bracketPairColorization: { enabled: true },
                        folding: true,
                        glyphMargin: false,
                        scrollbar: { verticalScrollbarSize: 8 },
                      }}
                    />
                  </div>
                </div>
                <div>
                  <label
                    style={{
                      fontSize: '13px',
                      fontWeight: 500,
                      color: 'var(--color-bobe-charcoal)',
                      display: 'block',
                      marginBottom: '4px',
                    }}
                  >
                    Excluded Tools (comma-separated)
                  </label>
                  <input
                    style={inputStyle}
                    placeholder="e.g. update_event, delete_event"
                    value={newExcludedTools}
                    onChange={(e) => setNewExcludedTools(e.target.value)}
                  />
                  <span
                    style={{
                      fontSize: '11px',
                      color: 'var(--color-bobe-clay)',
                      marginTop: '2px',
                      display: 'block',
                    }}
                  >
                    Tool names to hide from BoBe (server still exposes them)
                  </span>
                </div>
              </div>
            </div>

            <div className="settings-editor-toolbar">
              <div className="settings-editor-toolbar-left">
                <button
                  className="settings-button settings-button-secondary"
                  onClick={() => setCreating(false)}
                >
                  Cancel
                </button>
              </div>
              <div className="settings-editor-toolbar-right">
                <button
                  className="settings-button settings-button-primary"
                  onClick={handleCreate}
                  disabled={!newName.trim() || !newCommand.trim()}
                >
                  Add & Connect
                </button>
              </div>
            </div>
          </div>
        ) : selected ? (
          <div className="settings-editor">
            <div className="settings-editor-header">
              <div className="settings-editor-title">
                <span className="settings-editor-name">{selected.name}</span>
                {selected.connected ? (
                  <span
                    className="settings-list-item-badge"
                    style={{ background: 'var(--color-bobe-olive)', color: 'white' }}
                  >
                    connected
                  </span>
                ) : (
                  <span
                    className="settings-list-item-badge"
                    style={{ background: 'var(--color-bobe-terracotta)', color: 'white' }}
                  >
                    disconnected
                  </span>
                )}
                <span className="settings-list-item-badge">{selected.tool_count} tools</span>
              </div>
              {selected.error && (
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '6px',
                    marginTop: '6px',
                    fontSize: '12px',
                    color: 'var(--color-bobe-terracotta)',
                  }}
                >
                  <AlertCircle size={14} />
                  {selected.error}
                </div>
              )}
            </div>

            {/* Excluded tools section */}
            <div style={{ padding: '12px 16px', borderBottom: '1px solid var(--color-bobe-sand)' }}>
              <div
                style={{ display: 'flex', alignItems: 'center', gap: '6px', marginBottom: '8px' }}
              >
                <Ban size={14} style={{ color: 'var(--color-bobe-clay)' }} />
                <span
                  style={{ fontSize: '13px', fontWeight: 500, color: 'var(--color-bobe-charcoal)' }}
                >
                  Excluded Tools
                </span>
                <span style={{ fontSize: '11px', color: 'var(--color-bobe-clay)' }}>
                  (hidden from BoBe)
                </span>
              </div>
              {(selected.excluded_tools ?? []).length > 0 ? (
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', marginBottom: '8px' }}>
                  {(selected.excluded_tools ?? []).map((tool) => (
                    <span
                      key={tool}
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: '4px',
                        padding: '2px 8px',
                        borderRadius: '4px',
                        background: 'var(--color-bobe-sand)',
                        fontSize: '12px',
                        color: 'var(--color-bobe-charcoal)',
                        fontFamily: "Menlo, Monaco, 'Courier New', monospace",
                      }}
                    >
                      {tool}
                      <button
                        onClick={() => handleRemoveExcludedTool(selected.id, tool)}
                        disabled={updatingExcluded}
                        style={{
                          background: 'none',
                          border: 'none',
                          cursor: updatingExcluded ? 'default' : 'pointer',
                          padding: 0,
                          display: 'flex',
                          alignItems: 'center',
                          color: 'var(--color-bobe-clay)',
                          opacity: updatingExcluded ? 0.5 : 1,
                        }}
                        title={`Remove ${tool}`}
                      >
                        <X size={12} />
                      </button>
                    </span>
                  ))}
                </div>
              ) : (
                <p
                  style={{ fontSize: '12px', color: 'var(--color-bobe-clay)', marginBottom: '8px' }}
                >
                  No tools excluded
                </p>
              )}
              <div style={{ display: 'flex', gap: '6px', maxWidth: '350px' }}>
                <input
                  style={{ ...inputStyle, flex: 1 }}
                  placeholder="Tool name to exclude"
                  value={excludedInput}
                  onChange={(e) => setExcludedInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && excludedInput.trim()) {
                      handleAddExcludedTool(selected.id, excludedInput)
                    }
                  }}
                  disabled={updatingExcluded}
                />
                <button
                  className="settings-button settings-button-secondary settings-button-icon"
                  onClick={() => handleAddExcludedTool(selected.id, excludedInput)}
                  disabled={!excludedInput.trim() || updatingExcluded}
                  style={{ whiteSpace: 'nowrap' }}
                >
                  <Plus size={14} /> Add
                </button>
              </div>
            </div>

            {/* Config display as JSON */}
            <div className="settings-editor-monaco">
              <Editor
                height="100%"
                language="json"
                theme={monacoTheme}
                beforeMount={configureMonaco}
                value={JSON.stringify(
                  {
                    name: selected.name,
                    command: selected.command,
                    args: selected.args,
                    connected: selected.connected,
                    enabled: selected.enabled,
                    tool_count: selected.tool_count,
                    excluded_tools: selected.excluded_tools ?? [],
                  },
                  null,
                  2,
                )}
                options={{
                  readOnly: true,
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
                  scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10 },
                }}
              />
            </div>

            <div className="settings-editor-toolbar">
              <div className="settings-editor-toolbar-left">
                {deleteConfirm === selected.name ? (
                  <div className="settings-delete-confirm">
                    <span className="settings-delete-confirm-text">Remove server?</span>
                    <button
                      className="settings-button settings-button-danger"
                      onClick={() => handleDelete(selected.name)}
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
                    onClick={() => setDeleteConfirm(selected.name)}
                  >
                    <Trash2 size={14} /> Remove
                  </button>
                )}
              </div>
              <div className="settings-editor-toolbar-right">
                <button
                  className="settings-button settings-button-secondary settings-button-icon"
                  onClick={() => handleReconnect(selected.name)}
                  disabled={reconnecting === selected.name}
                >
                  <RefreshCw
                    size={14}
                    className={reconnecting === selected.name ? 'animate-spin' : ''}
                  />
                  {reconnecting === selected.name ? 'Reconnecting...' : 'Reconnect'}
                </button>
              </div>
            </div>
          </div>
        ) : (
          <div className="settings-editor-empty">
            <Server size={32} className="settings-editor-empty-icon" />
            <p className="settings-editor-empty-text">Select a server or add a new one</p>
          </div>
        )}
      </div>
    </>
  )
}
