/**
 * AIModelSettings component
 *
 * AI Model panel — provider picker, model management with explicit Save.
 * Changes to provider/model are buffered locally until Save is clicked.
 * Model downloads/deletes are immediate operations.
 */

import { useEffect, useState, useCallback, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { RefreshCw, AlertCircle, Download, Trash2, Check, ChevronDown, Save } from 'lucide-react'
import { cn } from '@/lib/cn'
import type { DaemonSettings, ModelInfo } from '@/types/api'

function getSettingsClient() {
  if (typeof window !== 'undefined' && 'settings' in window) {
    return window.settings
  }
  return null
}

export function AIModelSettings() {
  // Server state (source of truth)
  const [settings, setSettings] = useState<DaemonSettings | null>(null)
  const [models, setModels] = useState<ModelInfo[]>([])
  const [registryModels, setRegistryModels] = useState<ModelInfo[]>([])

  // Draft state (local edits before save)
  const [draftBackend, setDraftBackend] = useState<string | null>(null)
  const [draftModel, setDraftModel] = useState<string | null>(null)
  // Cloud provider drafts
  const [draftOpenAIKey, setDraftOpenAIKey] = useState<string | null>(null)
  const [draftOpenAIModel, setDraftOpenAIModel] = useState<string | null>(null)
  const [draftAzureEndpoint, setDraftAzureEndpoint] = useState<string | null>(null)
  const [draftAzureDeployment, setDraftAzureDeployment] = useState<string | null>(null)
  const [draftAzureKey, setDraftAzureKey] = useState<string | null>(null)

  // UI state
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [pulling, setPulling] = useState<string | null>(null)
  const [customModel, setCustomModel] = useState('')
  const [showTrending, setShowTrending] = useState(false)
  const [trendingLoading, setTrendingLoading] = useState(false)

  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    loadData()
    return () => {
      mountedRef.current = false
      // Clear sensitive data from memory on unmount
      setDraftOpenAIKey(null)
      setDraftAzureKey(null)
    }
  }, [])

  async function loadData() {
    setLoading(true)
    setError(null)
    try {
      const client = getSettingsClient()
      if (!client) {
        setError('Settings API not available')
        return
      }
      const [data, modelData] = await Promise.all([
        client.get(),
        client.listModels().catch(() => ({ models: [], backend: '', supports_pull: false })),
      ])
      if (!mountedRef.current) return
      setSettings(data)
      setModels(modelData.models || [])
    } catch (err) {
      if (mountedRef.current)
        setError(err instanceof Error ? err.message : 'Failed to load settings')
    } finally {
      if (mountedRef.current) setLoading(false)
    }
  }

  // Lazy-load trending when expanded
  async function loadTrending() {
    if (registryModels.length > 0) return // Already loaded
    setTrendingLoading(true)
    try {
      const client = getSettingsClient()
      if (client) {
        const data = await client
          .listRegistryModels()
          .catch(() => ({ models: [], backend: '', supports_pull: false }))
        if (mountedRef.current) setRegistryModels(data.models || [])
      }
    } catch {
      // Silently fail — trending is optional
    } finally {
      if (mountedRef.current) setTrendingLoading(false)
    }
  }

  // Derived: do we have unsaved changes?
  const activeBackend = draftBackend ?? settings?.llm_backend ?? 'ollama'
  const activeModel = draftModel ?? settings?.ollama_model ?? ''
  const isDirty =
    (draftBackend !== null && draftBackend !== settings?.llm_backend) ||
    (draftModel !== null && draftModel !== settings?.ollama_model) ||
    draftOpenAIKey !== null ||
    (draftOpenAIModel !== null && draftOpenAIModel !== settings?.openai_model) ||
    (draftAzureEndpoint !== null && draftAzureEndpoint !== settings?.azure_openai_endpoint) ||
    (draftAzureDeployment !== null && draftAzureDeployment !== settings?.azure_openai_deployment) ||
    draftAzureKey !== null

  // Save all buffered changes
  const handleSave = useCallback(async () => {
    if (!isDirty || !settings) return
    setSaving(true)
    setError(null)
    try {
      const client = getSettingsClient()
      if (!client) return
      const patch: Record<string, unknown> = {}
      if (draftBackend !== null && draftBackend !== settings.llm_backend)
        patch.llm_backend = draftBackend
      if (draftModel !== null && draftModel !== settings.ollama_model)
        patch.ollama_model = draftModel
      if (draftOpenAIKey !== null) patch.openai_api_key = draftOpenAIKey
      if (draftOpenAIModel !== null && draftOpenAIModel !== settings.openai_model)
        patch.openai_model = draftOpenAIModel
      if (draftAzureEndpoint !== null && draftAzureEndpoint !== settings.azure_openai_endpoint)
        patch.azure_openai_endpoint = draftAzureEndpoint
      if (
        draftAzureDeployment !== null &&
        draftAzureDeployment !== settings.azure_openai_deployment
      )
        patch.azure_openai_deployment = draftAzureDeployment
      if (draftAzureKey !== null) patch.azure_openai_api_key = draftAzureKey
      await client.update(patch)
      // Refresh to get server state
      const [data, modelData] = await Promise.all([
        client.get(),
        client.listModels().catch(() => ({ models: [], backend: '', supports_pull: false })),
      ])
      if (mountedRef.current) {
        setSettings(data)
        setModels(modelData.models || [])
        resetDrafts()
      }
    } catch (err) {
      if (mountedRef.current) setError(err instanceof Error ? err.message : 'Failed to save')
    } finally {
      if (mountedRef.current) setSaving(false)
    }
  }, [
    isDirty,
    settings,
    draftBackend,
    draftModel,
    draftOpenAIKey,
    draftOpenAIModel,
    draftAzureEndpoint,
    draftAzureDeployment,
    draftAzureKey,
  ])

  function resetDrafts() {
    setDraftBackend(null)
    setDraftModel(null)
    setDraftOpenAIKey(null)
    setDraftOpenAIModel(null)
    setDraftAzureEndpoint(null)
    setDraftAzureDeployment(null)
    setDraftAzureKey(null)
  }

  const handleDiscard = useCallback(() => {
    resetDrafts()
    setError(null)
  }, [])

  const handlePullModel = useCallback(async (modelName: string) => {
    if (!modelName.trim()) return
    const name = modelName.trim()
    setPulling(name)
    setError(null)
    try {
      const client = getSettingsClient()
      if (client) {
        await client.pullModel(name)
        if (!mountedRef.current) return
        const updated = await client.listModels().catch(() => ({ models: [] as ModelInfo[] }))
        if (mountedRef.current) setModels(updated.models || [])
      }
    } catch (err) {
      if (mountedRef.current)
        setError(err instanceof Error ? err.message : `Failed to download ${name}`)
    } finally {
      if (mountedRef.current) setPulling(null)
    }
  }, [])

  const handleDeleteModel = useCallback(async (modelName: string) => {
    setError(null)
    try {
      const client = getSettingsClient()
      if (client) {
        const result = await client.deleteModel(modelName)
        if (result.ok) {
          setModels((prev) => prev.filter((m) => m.name !== modelName))
        } else {
          setError(result.message)
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : `Failed to delete ${modelName}`)
    }
  }, [])

  if (loading) {
    return (
      <div className="preferences-panel">
        <div className="preferences-loading">
          <RefreshCw size={24} className="animate-spin" />
          <span>Loading model settings...</span>
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
          <button onClick={loadData} className="preferences-retry-btn">
            Retry
          </button>
        </div>
      </div>
    )
  }

  if (!settings) return null

  const isOllama = activeBackend === 'ollama' || activeBackend === 'local'
  const isOpenAI = activeBackend === 'openai'
  const isAzure = activeBackend === 'azure_openai'

  return (
    <div className="preferences-panel">
      {/* Save bar — appears when there are unsaved changes */}
      <AnimatePresence>
        {isDirty && (
          <motion.div
            className="ai-model-save-bar"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15 }}
          >
            <span className="ai-model-save-bar-text">Unsaved changes</span>
            <div className="ai-model-save-bar-actions">
              <button className="ai-model-discard-btn" onClick={handleDiscard} disabled={saving}>
                Discard
              </button>
              <button className="ai-model-save-btn" onClick={handleSave} disabled={saving}>
                {saving ? <RefreshCw size={14} className="animate-spin" /> : <Save size={14} />}
                {saving ? 'Saving...' : 'Save'}
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {error && (
        <div className="preferences-error" style={{ marginBottom: '1rem', padding: '12px 16px' }}>
          <AlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      {/* Provider Picker */}
      <div className="ai-model-provider-row">
        <span className="ai-model-provider-label">Provider</span>
        <select
          className="ai-model-provider-select"
          value={activeBackend}
          onChange={(e) => setDraftBackend(e.target.value)}
          disabled={saving}
        >
          <option value="ollama">Ollama (Local)</option>
          <option value="openai">OpenAI</option>
          <option value="azure_openai">Azure OpenAI</option>
          <option value="local">llama.cpp (Local)</option>
        </select>
      </div>

      {/* Ollama model management */}
      {isOllama && (
        <>
          {/* Active model selector */}
          <div className="ai-model-section">
            <h4 className="ai-model-section-title">Active Model</h4>
            {models.length > 0 ? (
              <select
                className="ai-model-select"
                value={activeModel}
                onChange={(e) => setDraftModel(e.target.value)}
                disabled={saving}
              >
                {models.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.name}
                  </option>
                ))}
              </select>
            ) : (
              <p className="ai-model-empty">No models installed — is Ollama running?</p>
            )}
          </div>

          {/* Installed models */}
          {models.length > 0 && (
            <div className="ai-model-section">
              <h4 className="ai-model-section-title">Installed Models</h4>
              <div className="ai-model-list">
                {models.map((m) => (
                  <div key={m.name} className="ai-model-item">
                    <div className="ai-model-item-info">
                      <span className="ai-model-item-name">
                        {m.name}
                        {m.name === settings.ollama_model && (
                          <span className="ai-model-active-badge">
                            <Check size={10} /> Active
                          </span>
                        )}
                      </span>
                      <span className="ai-model-item-size">{formatBytes(m.size_bytes)}</span>
                    </div>
                    {m.name !== settings.ollama_model && (
                      <div className="ai-model-item-actions">
                        <button
                          className="ai-model-use-btn"
                          onClick={() => setDraftModel(m.name)}
                          disabled={saving}
                        >
                          Use
                        </button>
                        <button
                          className="ai-model-delete-btn"
                          onClick={() => handleDeleteModel(m.name)}
                          title={`Delete ${m.name}`}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Download new model */}
          <div className="ai-model-section">
            <h4 className="ai-model-section-title">Download Model</h4>

            {pulling && (
              <div className="ai-model-pulling">
                <RefreshCw size={14} className="animate-spin" />
                <span>Downloading {pulling}...</span>
              </div>
            )}

            <div className="ai-model-download-row">
              <input
                type="text"
                value={customModel}
                onChange={(e) => setCustomModel(e.target.value)}
                placeholder="Model name (e.g. qwen3:14b)"
                className="ai-model-download-input"
                disabled={!!pulling}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    handlePullModel(customModel)
                    setCustomModel('')
                  }
                }}
              />
              <button
                className="ai-model-pull-btn"
                onClick={() => {
                  handlePullModel(customModel)
                  setCustomModel('')
                }}
                disabled={!customModel.trim() || !!pulling}
              >
                <Download size={14} /> Pull
              </button>
            </div>

            {/* Trending — lazy loaded, collapsible */}
            <button
              className="ai-model-trending-toggle"
              onClick={() => {
                const next = !showTrending
                setShowTrending(next)
                if (next) loadTrending()
              }}
              type="button"
            >
              <ChevronDown
                size={14}
                className={cn(
                  'ai-model-trending-chevron',
                  showTrending && 'ai-model-trending-chevron-open',
                )}
              />
              Browse trending models
            </button>
            <AnimatePresence>
              {showTrending && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.2 }}
                  style={{ overflow: 'hidden' }}
                >
                  {trendingLoading ? (
                    <div className="ai-model-pulling">
                      <RefreshCw size={14} className="animate-spin" />
                      <span>Loading trending models...</span>
                    </div>
                  ) : registryModels.length > 0 ? (
                    <div className="ai-model-trending-list">
                      {registryModels
                        .filter((rm) => !models.some((m) => m.name === rm.name))
                        .slice(0, 15)
                        .map((rm) => (
                          <div key={rm.name} className="ai-model-trending-item">
                            <span className="ai-model-trending-name">
                              {rm.name}
                              <span className="ai-model-trending-size">
                                {formatBytes(rm.size_bytes)}
                              </span>
                            </span>
                            <button
                              className="ai-model-trending-pull"
                              onClick={() => handlePullModel(rm.name)}
                              disabled={!!pulling}
                            >
                              <Download size={12} /> Pull
                            </button>
                          </div>
                        ))}
                    </div>
                  ) : (
                    <p className="ai-model-empty">No trending models available</p>
                  )}
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </>
      )}

      {/* OpenAI */}
      {isOpenAI && (
        <div className="ai-model-section">
          <h4 className="ai-model-section-title">OpenAI Configuration</h4>
          <div className="ai-model-form">
            <label className="ai-model-field">
              <span className="ai-model-field-label">API Key</span>
              <input
                type="password"
                className="ai-model-field-input"
                placeholder={
                  settings.openai_api_key_set ? 'Key saved (enter new to replace)' : 'sk-...'
                }
                value={draftOpenAIKey ?? ''}
                onChange={(e) => setDraftOpenAIKey(e.target.value || null)}
                disabled={saving}
                autoComplete="off"
              />
            </label>
            <label className="ai-model-field">
              <span className="ai-model-field-label">Model</span>
              <input
                type="text"
                className="ai-model-field-input"
                placeholder="gpt-4o-mini"
                value={draftOpenAIModel ?? settings.openai_model}
                onChange={(e) => setDraftOpenAIModel(e.target.value)}
                disabled={saving}
              />
            </label>
          </div>
        </div>
      )}

      {/* Azure OpenAI */}
      {isAzure && (
        <div className="ai-model-section">
          <h4 className="ai-model-section-title">Azure OpenAI Configuration</h4>
          <div className="ai-model-form">
            <label className="ai-model-field">
              <span className="ai-model-field-label">Endpoint</span>
              <input
                type="url"
                className="ai-model-field-input"
                placeholder="https://your-resource.openai.azure.com"
                value={draftAzureEndpoint ?? settings.azure_openai_endpoint}
                onChange={(e) => setDraftAzureEndpoint(e.target.value)}
                disabled={saving}
              />
            </label>
            <label className="ai-model-field">
              <span className="ai-model-field-label">API Key</span>
              <input
                type="password"
                className="ai-model-field-input"
                placeholder={
                  settings.azure_openai_api_key_set
                    ? 'Key saved (enter new to replace)'
                    : 'Enter API key'
                }
                value={draftAzureKey ?? ''}
                onChange={(e) => setDraftAzureKey(e.target.value || null)}
                disabled={saving}
                autoComplete="off"
              />
            </label>
            <label className="ai-model-field">
              <span className="ai-model-field-label">Deployment name</span>
              <input
                type="text"
                className="ai-model-field-input"
                placeholder="gpt-5-mini"
                value={draftAzureDeployment ?? settings.azure_openai_deployment}
                onChange={(e) => setDraftAzureDeployment(e.target.value)}
                disabled={saving}
              />
            </label>
          </div>
        </div>
      )}
    </div>
  )
}

// =============================================================================
// HELPERS
// =============================================================================

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}
