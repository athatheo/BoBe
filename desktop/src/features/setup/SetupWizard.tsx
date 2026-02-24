/**
 * Setup Wizard — thin UI client for the service's /onboarding/* API
 *
 * All setup logic lives in the Python service. This component just:
 * 1. Renders the UI (model selection, online config, progress)
 * 2. Calls IPC handlers that proxy to service endpoints
 * 3. Shows progress from the service's SSE model pull stream
 *
 * The frontend never stores API keys — they go straight to the service
 * which stores them in the OS keychain.
 */

import { useState, useCallback, useEffect } from 'react'
import type { MediaAccessStatus } from '@/types/ipc'

type SetupStep =
  | 'choose-mode'
  | 'no-internet'
  | 'downloading-engine'
  | 'downloading-model'
  | 'initializing'
  | 'permissions'
  | 'complete'
  | 'error'

interface ModelOption {
  id: string
  label: string
  size: string
  description: string
  modelName: string
}

const MODEL_OPTIONS: ModelOption[] = [
  {
    id: 'small',
    label: 'Small (3B)',
    size: '~2 GB',
    description: 'Fast, works on any Mac. Good for getting started.',
    modelName: 'llama3.2:3b',
  },
  {
    id: 'medium',
    label: 'Medium (14B)',
    size: '~8 GB',
    description: 'Smarter responses. Recommended for 32GB+ RAM.',
    modelName: 'qwen3:14b',
  },
  {
    id: 'large',
    label: 'Large (32B)',
    size: '~20 GB',
    description: 'Best quality. Recommended for 64GB+ RAM.',
    modelName: 'qwen3:32b',
  },
]

const ONLINE_PROVIDERS = [
  { id: 'openai', label: 'OpenAI', placeholder: 'sk-...', defaultModel: 'gpt-4o-mini' },
  {
    id: 'anthropic',
    label: 'Anthropic (Claude)',
    placeholder: 'sk-ant-...',
    defaultModel: 'claude-sonnet-4-5-20250929',
  },
  {
    id: 'azure_openai',
    label: 'Azure OpenAI',
    placeholder: 'Your Azure API key',
    defaultModel: 'gpt-5-mini',
    needsEndpoint: true,
    endpointPlaceholder: 'https://your-resource.cognitiveservices.azure.com/openai/v1/',
  },
]

// IPC bridge — calls preload's setup API which proxies to service /onboarding/* endpoints
const ipc = {
  startLocalSetup: (model: string): Promise<void> => window.setup.startLocalSetup(model),
  configureLLM: (
    mode: string,
    model: string,
    apiKey: string,
    endpoint?: string,
  ): Promise<{ ok: boolean; message: string }> =>
    window.setup.configureLLM(mode, model, apiKey, endpoint),
  completeSetup: (): Promise<void> => window.setup.completeSetup(),
  onProgress: (cb: (data: { step: string; progress: number; message: string }) => void) =>
    window.setup.onProgress(cb),
}

async function checkInternet(): Promise<boolean> {
  try {
    const resp = await fetch('https://github.com', {
      method: 'HEAD',
      signal: AbortSignal.timeout(5000),
    })
    return resp.ok
  } catch {
    return false
  }
}

function getSetupReason(): string {
  const params = new URLSearchParams(window.location.search)
  return params.get('reason') || 'first-run'
}

export function SetupWizard() {
  const reason = getSetupReason()
  const [step, setStep] = useState<SetupStep>('choose-mode')
  const [selectedModel, setSelectedModel] = useState<string>('small')
  const [showOnline, setShowOnline] = useState(false)
  const [onlineProvider, setOnlineProvider] = useState('openai')
  const [apiKey, setApiKey] = useState('')
  const [endpoint, setEndpoint] = useState('')
  const [onlineModel, setOnlineModel] = useState('gpt-4o-mini')
  const [progress, setProgress] = useState(0)
  const [statusMessage, setStatusMessage] = useState('')
  const [errorMessage, setErrorMessage] = useState('')
  const [setupMode, setSetupMode] = useState<'local' | 'online'>('local')
  const [busy, setBusy] = useState(false)

  // Local setup: check internet → IPC → service API
  const handleChooseLocal = useCallback(async () => {
    if (busy) return
    setBusy(true)

    const online = await checkInternet()
    if (!online) {
      setStep('no-internet')
      setBusy(false)
      return
    }

    const model = MODEL_OPTIONS.find((m) => m.id === selectedModel)
    if (!model) {
      setBusy(false)
      return
    }

    setSetupMode('local')
    setStep('downloading-engine')
    setStatusMessage('Downloading AI engine...')
    setProgress(0)

    // Listen for progress events from main process
    const unsubscribe = ipc.onProgress((data) => {
      setProgress(data.progress)
      setStatusMessage(data.message)
      if (data.step === 'model') setStep('downloading-model')
      if (data.step === 'init') setStep('initializing')
      if (data.step === 'complete') setStep('permissions')
    })

    try {
      await ipc.startLocalSetup(model.modelName)
      // Progress callback already transitions to 'permissions' on 'complete' event.
      // Only set here as a fallback if the event didn't fire.
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : 'Setup failed')
      setStep('error')
    } finally {
      unsubscribe?.()
      setBusy(false)
    }
  }, [selectedModel, busy])

  // Online setup: send API key to service (stores in keychain) → done
  const handleChooseOnline = useCallback(async () => {
    if (!apiKey.trim() || busy) return
    setBusy(true)

    try {
      // API key goes to service → keyring. Frontend forgets it immediately.
      const result = await ipc.configureLLM(onlineProvider, onlineModel, apiKey, endpoint || undefined)
      if (result && !result.ok) {
        setErrorMessage(result.message)
        setStep('error')
        return
      }
      setApiKey('') // Clear from memory
      setSetupMode('online')
      setStep('permissions')
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : 'Failed to save config')
      setStep('error')
    } finally {
      setBusy(false)
    }
  }, [onlineProvider, apiKey, onlineModel, busy])

  const handleRetry = useCallback(() => {
    setStep('choose-mode')
    setErrorMessage('')
    setProgress(0)
  }, [])

  const handleFinish = useCallback(() => {
    ipc.completeSetup()
  }, [])

  const subtitle =
    reason === 'missing-llm'
      ? 'No AI model configured. Choose how to connect.'
      : 'Your local AI companion. Choose how to get started.'

  return (
    <div style={containerStyle}>
      <h1 style={{ fontSize: '1.75rem', fontWeight: 600, marginBottom: '0.5rem' }}>
        {reason === 'missing-llm' ? 'Configure AI' : 'Welcome to BoBe'}
      </h1>
      <p style={{ color: '#8B9A7D', marginBottom: '1.5rem', textAlign: 'center', maxWidth: 380 }}>
        {subtitle}
      </p>

      {/* ============ CHOOSE MODE ============ */}
      {step === 'choose-mode' && (
        <div style={{ width: '100%', maxWidth: 440 }}>
          <p style={{ fontSize: '0.9rem', marginBottom: '0.75rem', fontWeight: 500 }}>
            Run AI locally on your Mac:
          </p>
          {MODEL_OPTIONS.map((model) => (
            <label key={model.id} style={radioCardStyle(selectedModel === model.id)}>
              <input
                type="radio"
                name="model"
                value={model.id}
                checked={selectedModel === model.id}
                onChange={() => setSelectedModel(model.id)}
                style={{ marginTop: '0.25rem', accentColor: '#C67B5C' }}
              />
              <div>
                <div style={{ fontWeight: 600 }}>
                  {model.label}{' '}
                  <span style={{ fontWeight: 400, color: '#A69080', fontSize: '0.85rem' }}>
                    {model.size}
                  </span>
                </div>
                <div style={{ fontSize: '0.8rem', color: '#666', marginTop: '0.15rem' }}>
                  {model.description}
                </div>
              </div>
            </label>
          ))}
          <button
            onClick={handleChooseLocal}
            disabled={busy}
            style={{
              ...primaryBtnStyle,
              opacity: busy ? 0.6 : 1,
              cursor: busy ? 'not-allowed' : 'pointer',
            }}
          >
            {busy ? 'Checking connection...' : 'Continue'}
          </button>

          <button
            onClick={() => setShowOnline(!showOnline)}
            style={{
              ...linkBtnStyle,
              marginTop: '1.25rem',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '0.35rem',
            }}
          >
            <span style={{ fontSize: '0.7rem' }}>{showOnline ? '\u25BC' : '\u25B6'}</span>
            {showOnline ? 'Hide online options' : 'Or use a cloud LLM instead'}
          </button>

          {showOnline && (
            <div style={collapsibleBoxStyle}>
              <p style={{ fontSize: '0.8rem', color: '#666', marginBottom: '0.75rem' }}>
                Connect to an online LLM. Your data stays on your Mac — only messages are sent to
                the API. The API key is stored in your OS keychain, never on disk.
              </p>
              <label style={fieldLabelStyle}>Provider</label>
              <select
                value={onlineProvider}
                onChange={(e) => {
                  setOnlineProvider(e.target.value)
                  const prov = ONLINE_PROVIDERS.find((p) => p.id === e.target.value)
                  if (prov) setOnlineModel(prov.defaultModel)
                }}
                style={inputStyle}
              >
                {ONLINE_PROVIDERS.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.label}
                  </option>
                ))}
              </select>
              <label style={{ ...fieldLabelStyle, marginTop: '0.5rem' }}>API Key</label>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder={ONLINE_PROVIDERS.find((p) => p.id === onlineProvider)?.placeholder}
                autoComplete="off"
                style={inputStyle}
              />
              {ONLINE_PROVIDERS.find((p) => p.id === onlineProvider)?.needsEndpoint && (
                <>
                  <label style={{ ...fieldLabelStyle, marginTop: '0.5rem' }}>Endpoint URL</label>
                  <input
                    type="text"
                    value={endpoint}
                    onChange={(e) => setEndpoint(e.target.value)}
                    placeholder={
                      ONLINE_PROVIDERS.find((p) => p.id === onlineProvider)?.endpointPlaceholder ??
                      'https://...'
                    }
                    style={inputStyle}
                  />
                </>
              )}
              <label style={{ ...fieldLabelStyle, marginTop: '0.5rem' }}>Model</label>
              <input
                type="text"
                value={onlineModel}
                onChange={(e) => setOnlineModel(e.target.value)}
                style={inputStyle}
              />
              <button
                onClick={handleChooseOnline}
                disabled={!apiKey.trim() || busy}
                style={{
                  ...primaryBtnStyle,
                  opacity: apiKey.trim() && !busy ? 1 : 0.5,
                  cursor: apiKey.trim() && !busy ? 'pointer' : 'not-allowed',
                }}
              >
                {busy ? 'Configuring...' : 'Continue with online LLM'}
              </button>
            </div>
          )}
        </div>
      )}

      {/* ============ NO INTERNET ============ */}
      {step === 'no-internet' && (
        <div style={{ width: '100%', maxWidth: 420, textAlign: 'center' }}>
          <div style={warningBoxStyle}>
            <p style={{ fontWeight: 600, marginBottom: '0.5rem', color: '#E65100' }}>
              No internet connection
            </p>
            <p style={{ fontSize: '0.85rem', color: '#555', lineHeight: 1.5 }}>
              BoBe needs internet only for the first setup to download an AI model to your Mac.
              After that, everything runs offline — no internet required.
            </p>
          </div>
          <button onClick={handleRetry} style={primaryBtnStyle}>
            Try again
          </button>
        </div>
      )}

      {/* ============ DOWNLOADING / INITIALIZING ============ */}
      {(step === 'downloading-engine' ||
        step === 'downloading-model' ||
        step === 'initializing') && (
        <div style={{ width: '100%', maxWidth: 420, textAlign: 'center' }}>
          <div style={{ marginBottom: '1rem', textAlign: 'left' }}>
            <StepIndicator
              label="Downloading AI engine"
              active={step === 'downloading-engine'}
              done={step === 'downloading-model' || step === 'initializing'}
            />
            <StepIndicator
              label="Downloading language model"
              active={step === 'downloading-model'}
              done={step === 'initializing'}
            />
            <StepIndicator
              label="Initializing BoBe"
              active={step === 'initializing'}
              done={false}
            />
          </div>
          <div style={progressTrackStyle}>
            <div style={{ ...progressFillStyle, width: `${progress}%` }} />
          </div>
          <p style={{ fontSize: '0.85rem', color: '#A69080' }}>{statusMessage}</p>
        </div>
      )}

      {/* ============ ERROR ============ */}
      {step === 'error' && (
        <div style={{ width: '100%', maxWidth: 420, textAlign: 'center' }}>
          <div style={errorBoxStyle}>
            <p style={{ fontWeight: 600, marginBottom: '0.5rem', color: '#C62828' }}>
              Setup failed
            </p>
            <p style={{ fontSize: '0.85rem', color: '#555', lineHeight: 1.5 }}>{errorMessage}</p>
          </div>
          <button onClick={handleRetry} style={primaryBtnStyle}>
            Retry
          </button>
        </div>
      )}

      {/* ============ PERMISSIONS ============ */}
      {step === 'permissions' && <PermissionsStep onContinue={() => setStep('complete')} />}

      {/* ============ COMPLETE ============ */}
      {step === 'complete' && (
        <div style={{ textAlign: 'center' }}>
          <p style={{ fontSize: '1.1rem', marginBottom: '0.5rem', color: '#8B9A7D' }}>
            All set! BoBe is ready.
          </p>
          <p style={{ fontSize: '0.8rem', color: '#A69080', marginBottom: '1.5rem' }}>
            {setupMode === 'online'
              ? 'Your data stays on your Mac — only messages are sent to the API.'
              : 'Everything runs locally on your Mac. No internet needed.'}
          </p>
          <button onClick={handleFinish} style={primaryBtnStyle}>
            Get Started
          </button>
        </div>
      )}
    </div>
  )
}

// =============================================================================
// PERMISSIONS STEP
// =============================================================================

function PermissionsStep({ onContinue }: { onContinue: () => void }) {
  const [dataDirOk, setDataDirOk] = useState<boolean | null>(null)
  const [dataDirError, setDataDirError] = useState('')
  const [screenStatus, setScreenStatus] = useState<MediaAccessStatus>('not-determined')

  const hasPermissionsApi =
    typeof window !== 'undefined' && 'permissions' in window && !!window.permissions

  // Check all permissions on mount
  useEffect(() => {
    if (!hasPermissionsApi) {
      // Not in Electron (e.g. browser dev mode) — skip checks
      setDataDirOk(true)
      setScreenStatus('granted')
      return
    }
    window.permissions
      .checkDataDir()
      .then((result) => {
        setDataDirOk(result.ok)
        if (!result.ok) setDataDirError(result.error || 'Unknown error')
      })
      .catch(() => setDataDirOk(true)) // Assume writable if IPC fails
    window.permissions
      .checkScreen()
      .then(setScreenStatus)
      .catch(() => {})
  }, [hasPermissionsApi])

  // Re-check screen permissions when the window regains focus
  // (user may have just granted permission in System Settings)
  useEffect(() => {
    if (!hasPermissionsApi) return

    function recheckPermissions() {
      window.permissions
        .checkScreen()
        .then(setScreenStatus)
        .catch(() => {})
    }

    window.addEventListener('focus', recheckPermissions)
    return () => window.removeEventListener('focus', recheckPermissions)
  }, [hasPermissionsApi])

  const handleRetryDataDir = () => {
    if (!hasPermissionsApi) return
    setDataDirOk(null)
    setDataDirError('')
    window.permissions
      .checkDataDir()
      .then((result) => {
        setDataDirOk(result.ok)
        if (!result.ok) setDataDirError(result.error || 'Unknown error')
      })
      .catch(() => setDataDirOk(true))
  }

  // Fatal: data directory not writable
  if (dataDirOk === false) {
    return (
      <div style={{ width: '100%', maxWidth: 420, textAlign: 'center' }}>
        <div style={errorBoxStyle}>
          <p style={{ fontWeight: 600, marginBottom: '0.5rem', color: '#C62828' }}>
            Cannot create data folder
          </p>
          <p style={{ fontSize: '0.85rem', color: '#555', lineHeight: 1.5 }}>
            BoBe needs a folder in your home directory to store its data. Please check that your
            disk is not full and that file permissions allow writing.
          </p>
          {dataDirError && (
            <p
              style={{
                fontSize: '0.75rem',
                color: '#999',
                marginTop: '0.5rem',
                fontFamily: 'monospace',
                wordBreak: 'break-all',
              }}
            >
              {dataDirError}
            </p>
          )}
        </div>
        <button onClick={handleRetryDataDir} style={primaryBtnStyle}>
          Retry
        </button>
        <button onClick={() => window.close()} style={{ ...linkBtnStyle, marginTop: '0.5rem' }}>
          Quit BoBe
        </button>
      </div>
    )
  }

  // Still loading
  if (dataDirOk === null) {
    return (
      <div style={{ width: '100%', maxWidth: 420, textAlign: 'center' }}>
        <p style={{ fontSize: '0.9rem', color: '#A69080' }}>Checking permissions...</p>
      </div>
    )
  }

  return (
    <div style={{ width: '100%', maxWidth: 440 }}>
      <p style={{ fontSize: '0.9rem', marginBottom: '1rem', fontWeight: 500 }}>
        BoBe works best with these permissions:
      </p>

      {/* Screen Recording */}
      <div style={permissionCardStyle}>
        <div style={permissionHeaderStyle}>
          <span style={{ fontWeight: 600 }}>Screen Recording</span>
          <PermissionBadge status={screenStatus} />
        </div>
        <p style={permissionDescStyle}>Lets BoBe see your screen to provide contextual help.</p>
        {screenStatus === 'restricted' && (
          <p style={permissionNoteStyle}>
            This permission is managed by your organization and cannot be changed.
          </p>
        )}
        {screenStatus !== 'granted' && screenStatus !== 'restricted' && (
          <>
            <button
              onClick={() => hasPermissionsApi && window.permissions.openScreenSettings()}
              style={settingsLinkStyle}
            >
              Open System Settings
            </button>
            <p style={permissionNoteStyle}>
              Screen capture will take effect after you finish setup and restart BoBe.
            </p>
          </>
        )}
      </div>

      <p style={{ fontSize: '0.8rem', color: '#A69080', marginTop: '1rem', lineHeight: 1.5 }}>
        These permissions are optional. You can grant them later in Settings.
      </p>

      <button onClick={onContinue} style={{ ...primaryBtnStyle, marginTop: '1rem' }}>
        Continue
      </button>
    </div>
  )
}

function PermissionBadge({ status, loading }: { status: MediaAccessStatus; loading?: boolean }) {
  if (loading) {
    return <span style={badgeStyle('#A69080', '#F5F0EA')}>Requesting...</span>
  }
  switch (status) {
    case 'granted':
      return <span style={badgeStyle('#2E7D32', '#E8F5E9')}>{'\u2713'} Granted</span>
    case 'denied':
      return <span style={badgeStyle('#C62828', '#FFEBEE')}>Not Granted</span>
    case 'restricted':
      return <span style={badgeStyle('#E65100', '#FFF3E0')}>Restricted</span>
    case 'not-determined':
      return <span style={badgeStyle('#A69080', '#F5F0EA')}>Not Set</span>
  }
}

function StepIndicator({ label, active, done }: { label: string; active: boolean; done: boolean }) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '0.5rem',
        padding: '0.4rem 0',
        fontSize: '0.9rem',
        color: active ? '#3A3A3A' : done ? '#8B9A7D' : '#A69080',
        fontWeight: active ? 600 : 400,
      }}
    >
      <span>{done ? '\u2713' : active ? '\u25CF' : '\u25CB'}</span>
      <span>{label}</span>
    </div>
  )
}

// --- Styles ---
const containerStyle: React.CSSProperties = {
  fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Rounded", sans-serif',
  background: '#FAF7F2',
  color: '#3A3A3A',
  height: '100vh',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'flex-start',
  padding: '2rem',
  paddingTop: '3.5rem',
  userSelect: 'none',
  overflowY: 'auto',
}
function radioCardStyle(selected: boolean): React.CSSProperties {
  return {
    display: 'flex',
    alignItems: 'flex-start',
    gap: '0.75rem',
    padding: '0.65rem 0.85rem',
    marginBottom: '0.4rem',
    borderRadius: '0.75rem',
    border: `2px solid ${selected ? '#C67B5C' : '#E8DCC4'}`,
    background: selected ? '#FFF8F4' : 'white',
    cursor: 'pointer',
    transition: 'all 0.15s',
  }
}
const primaryBtnStyle: React.CSSProperties = {
  width: '100%',
  marginTop: '0.5rem',
  padding: '0.7rem',
  borderRadius: '0.75rem',
  border: 'none',
  background: '#C67B5C',
  color: 'white',
  fontSize: '0.95rem',
  fontWeight: 600,
  cursor: 'pointer',
}
const linkBtnStyle: React.CSSProperties = {
  width: '100%',
  padding: '0.5rem',
  borderRadius: '0.75rem',
  border: 'none',
  background: 'transparent',
  color: '#A69080',
  fontSize: '0.85rem',
  cursor: 'pointer',
}
const fieldLabelStyle: React.CSSProperties = {
  fontSize: '0.8rem',
  fontWeight: 500,
  display: 'block',
  marginBottom: '0.25rem',
}
const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '0.5rem 0.65rem',
  borderRadius: '0.5rem',
  border: '1px solid #E8DCC4',
  fontSize: '0.85rem',
  background: 'white',
  boxSizing: 'border-box',
}
const collapsibleBoxStyle: React.CSSProperties = {
  marginTop: '0.75rem',
  padding: '1rem',
  background: '#F5F0EA',
  borderRadius: '0.75rem',
  border: '1px solid #E8DCC4',
}
const warningBoxStyle: React.CSSProperties = {
  background: '#FFF3E0',
  border: '1px solid #FFB74D',
  borderRadius: '0.75rem',
  padding: '1.25rem',
  marginBottom: '1rem',
}
const errorBoxStyle: React.CSSProperties = {
  background: '#FFEBEE',
  border: '1px solid #EF9A9A',
  borderRadius: '0.75rem',
  padding: '1.25rem',
  marginBottom: '1rem',
}
const progressTrackStyle: React.CSSProperties = {
  width: '100%',
  height: 8,
  background: '#E8DCC4',
  borderRadius: 4,
  overflow: 'hidden',
  marginBottom: '0.75rem',
}
const progressFillStyle: React.CSSProperties = {
  height: '100%',
  background: '#C67B5C',
  borderRadius: 4,
  transition: 'width 0.3s ease',
}
const permissionCardStyle: React.CSSProperties = {
  padding: '0.85rem 1rem',
  marginBottom: '0.5rem',
  borderRadius: '0.75rem',
  border: '1px solid #E8DCC4',
  background: 'white',
}
const permissionHeaderStyle: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  marginBottom: '0.25rem',
}
const permissionDescStyle: React.CSSProperties = {
  fontSize: '0.8rem',
  color: '#666',
  lineHeight: 1.4,
}
const permissionNoteStyle: React.CSSProperties = {
  fontSize: '0.75rem',
  color: '#A69080',
  marginTop: '0.35rem',
  fontStyle: 'italic',
}
const settingsLinkStyle: React.CSSProperties = {
  background: 'none',
  border: 'none',
  color: '#C67B5C',
  fontSize: '0.8rem',
  fontWeight: 500,
  cursor: 'pointer',
  padding: 0,
  marginTop: '0.35rem',
  textDecoration: 'underline',
}
function badgeStyle(color: string, bg: string): React.CSSProperties {
  return {
    fontSize: '0.75rem',
    fontWeight: 500,
    color,
    background: bg,
    padding: '0.15rem 0.5rem',
    borderRadius: '0.5rem',
  }
}
