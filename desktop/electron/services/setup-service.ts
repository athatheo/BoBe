/**
 * SetupService - Thin client for the service's onboarding REST API
 *
 * The service owns all setup logic (what's configured, what's missing,
 * LLM configuration, model downloads, API key storage in keychain).
 * This module just:
 * 1. Manages the setup BrowserWindow
 * 2. Proxies IPC calls from the renderer to the service's /onboarding/* endpoints
 * 3. Handles Ollama binary download (platform concern — different URL per OS)
 * 4. Manages the DB encryption key (Electron's safeStorage)
 *
 * Setup is mandatory on first run. Closing the window quits the app.
 */

import { app, BrowserWindow, ipcMain, safeStorage } from 'electron'
import { existsSync, readFileSync, statfsSync, writeFileSync } from 'node:fs'
import { randomBytes } from 'node:crypto'
import path from 'node:path'
import { ollamaService } from './ollama-service'

// Minimum disk space required per model (bytes), including Ollama binary overhead
const MODEL_DISK_REQUIREMENTS: Record<string, number> = {
  'llama3.2:3b': 2.5e9, // ~2.5 GB
  'qwen3:14b': 9e9, // ~9 GB
  'qwen3:32b': 22e9, // ~22 GB
}
const DEFAULT_DISK_REQUIREMENT = 5e9 // 5 GB fallback for unknown models

const SERVICE_URL = 'http://127.0.0.1:8766'
const ENCRYPTED_KEY_FILE = 'db.key'

// =============================================================================
// ENCRYPTION KEY (Signal Desktop pattern)
// =============================================================================

function getKeyPath(): string {
  return path.join(app.getPath('userData'), ENCRYPTED_KEY_FILE)
}

export function ensureEncryptionKey(): string {
  const existing = getEncryptionKey()
  if (existing) return existing

  const key = randomBytes(32).toString('hex')

  if (!safeStorage.isEncryptionAvailable()) {
    console.warn(
      '[Setup] WARNING: safeStorage not available. Encryption key stored in plaintext.',
      'On Linux, install gnome-keyring or kwallet for secure storage.',
      'API keys will NOT be encrypted at rest.',
    )
    writeFileSync(getKeyPath(), key)
    return key
  }

  const encrypted = safeStorage.encryptString(key)
  writeFileSync(getKeyPath(), encrypted)
  console.log('[Setup] Encryption key generated and stored in OS keychain')
  return key
}

export function getEncryptionKey(): string | null {
  const keyPath = getKeyPath()
  if (!existsSync(keyPath)) return null
  try {
    const data = readFileSync(keyPath)
    if (!safeStorage.isEncryptionAvailable()) return data.toString('utf-8')
    return safeStorage.decryptString(data)
  } catch {
    return null
  }
}

export function hasEncryptionKey(): boolean {
  return getEncryptionKey() !== null
}

// =============================================================================
// ONBOARDING STATUS (delegates to service)
// =============================================================================

interface OnboardingStatus {
  complete: boolean
  needs_onboarding: boolean
  steps: Record<string, { status: string; detail: string }>
}

/** Ask the service if onboarding is needed. Returns null if service unreachable. */
export async function checkOnboardingStatus(): Promise<OnboardingStatus | null> {
  try {
    const resp = await fetch(`${SERVICE_URL}/onboarding/status`, {
      signal: AbortSignal.timeout(3000),
    })
    if (!resp.ok) return null
    return (await resp.json()) as OnboardingStatus
  } catch {
    return null
  }
}

// =============================================================================
// SETUP WIZARD WINDOW
// =============================================================================

let setupWindow: BrowserWindow | null = null

/**
 * Open the setup wizard and wait for completion.
 * The wizard's IPC calls proxy to the service's /onboarding/* endpoints.
 */
export async function runSetupWizard(
  reason: 'first-run' | 'missing-llm' | 'user-requested' = 'first-run',
): Promise<void> {
  // Ensure encryption key exists before any setup
  ensureEncryptionKey()

  return new Promise<void>((resolve) => {
    let resolved = false
    const done = () => {
      if (!resolved) {
        resolved = true
        cleanup()
        resolve()
      }
    }

    setupWindow = new BrowserWindow({
      width: 540,
      height: 620,
      center: true,
      resizable: false,
      minimizable: false,
      closable: true,
      titleBarStyle: 'hiddenInset',
      trafficLightPosition: { x: 12, y: 12 },
      webPreferences: {
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true,
        preload: path.join(__dirname, '../preload/index.js'),
      },
    })

    if (process.env.ELECTRON_RENDERER_URL) {
      setupWindow.loadURL(`${process.env.ELECTRON_RENDERER_URL}/setup.html?reason=${reason}`)
    } else {
      setupWindow.loadFile(path.join(__dirname, '../renderer/setup.html'), {
        query: { reason },
      })
    }

    // --- IPC handlers: proxy to service API ---

    // Configure LLM (local or online) — service stores in keyring + .env
    ipcMain.handle(
      'bobe:configure-llm',
      async (_event, mode: string, model: string, apiKey: string) => {
        const resp = await fetch(`${SERVICE_URL}/onboarding/configure-llm`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ mode, model, api_key: apiKey }),
        })
        return resp.json()
      },
    )

    // Start local setup: download Ollama binary (platform) + configure + pull model (service)
    ipcMain.handle('bobe:start-local-setup', async (_event, modelName: string) => {
      // Pre-check: disk space
      const requiredBytes = MODEL_DISK_REQUIREMENTS[modelName] ?? DEFAULT_DISK_REQUIREMENT
      const requiredGB = (requiredBytes / 1e9).toFixed(1)
      try {
        const stats = statfsSync(app.getPath('userData'))
        const availableBytes = stats.bfree * stats.bsize
        if (availableBytes < requiredBytes) {
          const availableGB = (availableBytes / 1e9).toFixed(1)
          throw new Error(
            `Not enough disk space. Need ~${requiredGB} GB free, but only ${availableGB} GB available.`,
          )
        }
      } catch (e) {
        // Re-throw disk space errors, ignore statfs failures (proceed anyway)
        if (e instanceof Error && e.message.includes('disk space')) throw e
      }

      // Step 1: Ensure Ollama binary is installed (Electron's job — platform-specific)
      sendProgress('engine', 0, 'Downloading AI engine...')
      const onEngineProgress = (percent: number, downloadedMB: number, totalMB: number): void => {
        sendProgress(
          'engine',
          percent,
          `Downloading AI engine: ${percent}% (${downloadedMB.toFixed(0)} / ${totalMB.toFixed(0)} MB)`,
        )
      }
      ollamaService.on('download-progress', onEngineProgress)
      try {
        await ollamaService.ensureInstalled()
        sendProgress('engine', 100, 'AI engine ready')
      } catch (error) {
        sendProgress('engine', 0, `Failed to download AI engine: ${error}`)
        throw error
      } finally {
        ollamaService.off('download-progress', onEngineProgress)
      }

      // Step 2: Configure LLM via service API
      sendProgress('model', 0, 'Configuring...')
      try {
        const configResp = await fetch(`${SERVICE_URL}/onboarding/configure-llm`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ mode: 'ollama', model: modelName }),
        })
        if (!configResp.ok) throw new Error('Failed to configure LLM')
      } catch (error) {
        sendProgress('model', 0, `Configuration failed: ${error}`)
        throw error
      }

      // Step 3: Pull model via service SSE stream
      sendProgress('model', 0, `Downloading ${modelName}...`)
      try {
        await pullModelViaService(modelName)
        sendProgress('model', 100, 'Model ready')
      } catch (error) {
        sendProgress('model', 0, `Failed to download model: ${error}`)
        throw error
      }

      // Step 4: Warm up embedding model (pulls ~274 MB nomic-embed-text via Ollama if not cached)
      sendProgress('init', 50, 'Downloading embedding model...')
      try {
        const warmupResp = await fetch(`${SERVICE_URL}/onboarding/warmup-embedding`, {
          method: 'POST',
          signal: AbortSignal.timeout(120_000), // 2 min — first download can be slow
        })
        if (warmupResp.ok) {
          sendProgress('init', 100, 'Embedding model ready')
        }
      } catch {
        // Non-fatal — embedding will download on first use
        console.warn('[Setup] Embedding warmup timed out, will download on first use')
      }

      sendProgress('complete', 100, 'Setup complete!')
    })

    // Get onboarding status from service
    ipcMain.handle('bobe:get-onboarding-status', async () => {
      return checkOnboardingStatus()
    })

    // Setup complete — warm up embedding (if not done already) + signal service + close window
    ipcMain.handle('bobe:complete-setup', async () => {
      // Trigger embedding warmup in background (non-blocking for online setup path)
      fetch(`${SERVICE_URL}/onboarding/warmup-embedding`, {
        method: 'POST',
        signal: AbortSignal.timeout(120_000),
      }).catch(() => {
        /* best-effort */
      })

      try {
        await fetch(`${SERVICE_URL}/onboarding/mark-complete`, { method: 'POST' })
      } catch {
        // Service might not be reachable — that's OK
      }
      if (setupWindow && !setupWindow.isDestroyed()) {
        setupWindow.close()
      }
      done()
    })

    // Closing window = quit (setup is mandatory)
    setupWindow.on('closed', () => {
      setupWindow = null
      if (!resolved) {
        app.quit()
      }
      done()
    })

    function cleanup(): void {
      ipcMain.removeHandler('bobe:configure-llm')
      ipcMain.removeHandler('bobe:start-local-setup')
      ipcMain.removeHandler('bobe:get-onboarding-status')
      ipcMain.removeHandler('bobe:complete-setup')
      if (setupWindow && !setupWindow.isDestroyed()) {
        setupWindow.close()
      }
      setupWindow = null
    }
  })
}

// =============================================================================
// Model pull via service API (SSE stream)
// =============================================================================

async function pullModelViaService(modelName: string): Promise<void> {
  const resp = await fetch(`${SERVICE_URL}/onboarding/pull-model`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model: modelName }),
  })

  if (!resp.ok || !resp.body) {
    throw new Error(`Pull model failed: ${resp.status}`)
  }

  const reader = resp.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() || ''

    for (const line of lines) {
      if (!line.startsWith('data: ')) continue
      try {
        const data = JSON.parse(line.slice(6)) as {
          status: string
          progress?: number
          downloaded?: string
          total?: string
          detail?: string
        }
        if (data.status === 'pulling' && data.progress !== undefined) {
          sendProgress(
            'model',
            data.progress,
            `Downloading: ${data.progress}% (${data.downloaded} / ${data.total})`,
          )
        } else if (data.status === 'complete') {
          return
        } else if (data.status === 'error') {
          throw new Error(data.detail || 'Model download failed')
        }
      } catch (e) {
        if (e instanceof Error && e.message !== 'Model download failed') continue
        throw e
      }
    }
  }
}

function sendProgress(step: string, progress: number, message: string): void {
  if (setupWindow && !setupWindow.isDestroyed()) {
    setupWindow.webContents.send('bobe:setup-progress', { step, progress, message })
  }
}
