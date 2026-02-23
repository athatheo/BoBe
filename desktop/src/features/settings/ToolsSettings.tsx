/**
 * ToolsSettings component
 *
 * List available tools with enable/disable toggles.
 * Tools are read-only (can't create/delete), only toggle enabled state.
 * Descriptions are truncated with click-to-expand.
 */

import { useEffect, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Wrench, RefreshCw, ChevronDown, ChevronUp, Search } from 'lucide-react'
import { cn } from '@/lib/cn'
import { getToolsClient } from '@/lib/browser-settings-client'
import type { Tool } from '@/types/api'

const DESC_TRUNCATE = 80

function ToolRow({ tool, onToggle }: { tool: Tool; onToggle: () => void }) {
  const [expanded, setExpanded] = useState(false)
  const long = (tool.description?.length ?? 0) > DESC_TRUNCATE

  return (
    <div
      className="settings-list-item"
      style={{
        cursor: long ? 'pointer' : 'default',
        flexDirection: 'column',
        alignItems: 'stretch',
        gap: 0,
      }}
      onClick={() => long && setExpanded((p) => !p)}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          width: '100%',
        }}
      >
        <div className="settings-list-item-info" style={{ minWidth: 0 }}>
          <div className="settings-list-item-main">
            <span className="settings-list-item-name">{tool.name}</span>
            {tool.category && <span className="settings-list-item-badge">{tool.category}</span>}
          </div>
          {tool.description && !expanded && (
            <span
              className="settings-list-item-meta"
              style={{ display: 'flex', alignItems: 'center', gap: '4px' }}
            >
              {long ? tool.description.slice(0, DESC_TRUNCATE) + '...' : tool.description}
              {long && (
                <ChevronDown size={12} style={{ flexShrink: 0, color: 'var(--color-bobe-clay)' }} />
              )}
            </span>
          )}
        </div>
        <div className="settings-list-item-actions" onClick={(e) => e.stopPropagation()}>
          <button
            type="button"
            role="switch"
            aria-checked={tool.enabled}
            onClick={onToggle}
            className={cn(
              'settings-toggle',
              tool.enabled ? 'settings-toggle-on' : 'settings-toggle-off',
            )}
          >
            <motion.span
              className="settings-toggle-thumb"
              animate={{ x: tool.enabled ? 18 : 2 }}
              transition={{ type: 'spring', stiffness: 500, damping: 30 }}
            />
          </button>
        </div>
      </div>
      <AnimatePresence>
        {expanded && tool.description && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15 }}
            style={{ overflow: 'hidden' }}
          >
            <p
              style={{
                fontSize: '12px',
                lineHeight: 1.5,
                color: 'var(--color-bobe-charcoal)',
                margin: '8px 0 4px',
                padding: '8px 12px',
                background: 'var(--color-bobe-warm-white)',
                borderRadius: '6px',
                border: '1px solid var(--color-bobe-sand)',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
              }}
            >
              {tool.description}
            </p>
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '4px',
                fontSize: '11px',
                color: 'var(--color-bobe-clay)',
              }}
            >
              <ChevronUp size={12} /> collapse
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

export function ToolsSettings() {
  const [tools, setTools] = useState<Tool[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const client = getToolsClient()

  useEffect(() => {
    loadTools()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only
  }, [])

  async function loadTools() {
    setLoading(true)
    setError(null)
    try {
      const res = await client.list()
      setTools(res.tools)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load tools')
    } finally {
      setLoading(false)
    }
  }

  async function handleToggle(tool: Tool) {
    try {
      if (tool.enabled) {
        await client.disable(tool.name)
      } else {
        await client.enable(tool.name)
      }
      setTools((prev) =>
        prev.map((t) => (t.name === tool.name ? { ...t, enabled: !t.enabled } : t)),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to toggle tool')
    }
  }

  // Group by provider
  const byProvider = tools.reduce<Record<string, Tool[]>>((acc, tool) => {
    const key = tool.provider || 'native'
    if (!acc[key]) acc[key] = []
    acc[key].push(tool)
    return acc
  }, {})

  if (loading) {
    return (
      <div className="settings-editor-loading">
        <div className="settings-editor-loading-spinner" />
        <span>Loading tools...</span>
      </div>
    )
  }

  return (
    <div style={{ flex: 1, overflowY: 'auto', padding: '0' }}>
      {error && <div className="settings-error">{error}</div>}

      <div
        style={{
          padding: '12px 16px',
          borderBottom: '1px solid var(--color-bobe-sand)',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}
      >
        <span style={{ fontSize: '13px', color: 'var(--color-bobe-clay)' }}>
          {tools.filter((t) => t.enabled).length} of {tools.length} enabled
        </span>
        <div style={{ display: 'flex', gap: '6px' }}>
          <button
            className="settings-button settings-button-secondary settings-button-icon"
            disabled
            title="Coming soon — inspect Homebrew and other tools on your Mac"
            style={{ opacity: 0.5, cursor: 'not-allowed' }}
          >
            <Search size={14} /> Discover my Work
          </button>
          <button
            className="settings-button settings-button-secondary settings-button-icon"
            onClick={loadTools}
          >
            <RefreshCw size={14} /> Refresh
          </button>
        </div>
      </div>

      {tools.length === 0 ? (
        <div className="settings-empty-state">
          <Wrench size={32} className="settings-empty-icon" />
          <p className="settings-empty-text">No tools available</p>
          <p className="settings-empty-hint">Tools become available when the daemon is running</p>
        </div>
      ) : (
        Object.entries(byProvider).map(([provider, providerTools]) => (
          <div key={provider}>
            <div
              style={{
                padding: '8px 16px',
                background: 'var(--color-bobe-sand)',
                fontSize: '11px',
                fontWeight: 600,
                color: 'var(--color-bobe-clay)',
                textTransform: 'uppercase',
                letterSpacing: '0.05em',
              }}
            >
              {provider}
            </div>
            <ul className="settings-item-list">
              {providerTools.map((tool) => (
                <li key={tool.name}>
                  <ToolRow tool={tool} onToggle={() => handleToggle(tool)} />
                </li>
              ))}
            </ul>
          </div>
        ))
      )}
    </div>
  )
}
