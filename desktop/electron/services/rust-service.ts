/**
 * BackendService - Manages connectivity to the Rust/Axum backend
 *
 * Two modes:
 *   1. External (dev): assumes `bobe serve` is running externally; just health-checks
 *   2. Managed (packaged): spawns the bundled `bobe` binary from Resources/
 *
 * Same EventEmitter interface as the old PythonService:
 *   state: stopped → starting → ready | crashed | fatal
 */

import { app } from 'electron'
import { type ChildProcess, spawn } from 'node:child_process'
import { existsSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import path from 'node:path'
import { EventEmitter } from 'node:events'

const HEALTH_URL = 'http://127.0.0.1:8766/health'
const MAX_HEALTH_ATTEMPTS = 20
const HEALTH_INITIAL_DELAY_MS = 300
const HEALTH_MAX_DELAY_MS = 3000
const SHUTDOWN_TIMEOUT_MS = 5000

export type BackendServiceState = 'stopped' | 'starting' | 'ready' | 'crashed' | 'fatal'

export interface BackendServiceEvents {
  ready: []
  crashed: [exitCode: number | null]
  fatal: []
  state: [state: BackendServiceState]
}

export class BackendService extends EventEmitter<BackendServiceEvents> {
  private process: ChildProcess | null = null
  private _state: BackendServiceState = 'stopped'
  private stopping = false
  private managed = false // true when we spawned the process ourselves
  private appDataDir: string

  constructor() {
    super()
    this.appDataDir = app.getPath('userData')
  }

  get state(): BackendServiceState {
    return this._state
  }

  /** Start the backend service (or connect to an already-running one) */
  async start(): Promise<void> {
    if (this._state === 'starting' || this._state === 'ready') {
      console.log('[BackendService] Already running, skipping start')
      return
    }

    this.stopping = false
    this.setState('starting')

    // Try to spawn the bundled binary if available
    const binaryPath = this.getBinaryPath()
    if (binaryPath) {
      try {
        this.spawnProcess(binaryPath)
        this.managed = true
      } catch (error) {
        console.error('[BackendService] Failed to spawn process:', error)
        this.setState('fatal')
        this.emit('fatal')
        return
      }
    } else {
      console.log('[BackendService] No bundled binary found — assuming external backend')
      this.managed = false
    }

    await this.waitForHealth()
  }

  /** Stop the backend service. Safe to call even if never started. */
  async stop(): Promise<void> {
    this.stopping = true

    if (!this.managed || !this.process) {
      this.process = null
      this.setState('stopped')
      return
    }

    const pid = this.process.pid
    console.log(`[BackendService] Stopping process ${pid}...`)
    this.setState('stopped')

    this.process.kill('SIGTERM')

    await new Promise<void>((resolve) => {
      const forceKillTimer = setTimeout(() => {
        if (this.process && !this.process.killed) {
          console.log(`[BackendService] Force killing process ${pid}`)
          try {
            this.process.kill('SIGKILL')
          } catch {
            // Process may have already exited
          }
        }
        resolve()
      }, SHUTDOWN_TIMEOUT_MS)

      if (this.process) {
        this.process.once('exit', () => {
          clearTimeout(forceKillTimer)
          resolve()
        })
      } else {
        clearTimeout(forceKillTimer)
        resolve()
      }
    })

    this.process = null
    console.log('[BackendService] Stopped')
  }

  /** Resolve the bundled Rust binary path, or null if not found */
  private getBinaryPath(): string | null {
    // Packaged app: binary lives next to the app resources
    const bundledPath = path.join(process.resourcesPath, 'bin', 'bobe')
    if (existsSync(bundledPath)) {
      return bundledPath
    }

    if (app.isPackaged) {
      console.error(
        '[BackendService] Bundled binary not found in packaged app. ' +
          'Expected at: ' +
          bundledPath,
      )
    }

    // Dev mode: no bundled binary — caller should run `bobe serve` externally
    return null
  }

  /** Spawn the Rust backend process */
  private spawnProcess(binaryPath: string): void {
    const dataDir = this.appDataDir
    const logsDir = path.join(dataDir, 'logs')
    const dbDir = path.join(dataDir, 'data')
    const modelsDir = path.join(dataDir, 'models')
    const ollamaBinDir = path.join(dataDir, 'ollama', 'bin')

    mkdirSync(logsDir, { recursive: true })
    mkdirSync(dbDir, { recursive: true })
    mkdirSync(modelsDir, { recursive: true })

    const dbPath = path.join(dbDir, 'bobe.db')

    const existingPath = process.env['PATH'] || '/usr/local/bin:/usr/bin:/bin'
    const augmentedPath = `${ollamaBinDir}:${existingPath}`
    const ollamaBinPath = path.join(ollamaBinDir, 'ollama')

    const env: Record<string, string> = {
      ...(process.env as Record<string, string>),
      PATH: augmentedPath,
      BOBE_HOST: '127.0.0.1',
      BOBE_PORT: '8766',
      BOBE_DATABASE_URL: `sqlite:///${dbPath}`,
      BOBE_OLLAMA_AUTO_START: 'true',
      BOBE_OLLAMA_AUTO_PULL: 'true',
      BOBE_OLLAMA_BINARY_PATH: ollamaBinPath,
      BOBE_LOG_FILE: path.join(logsDir, 'bobe-service.log'),
      BOBE_LOG_LEVEL: 'info',
      OLLAMA_HOST: '127.0.0.1:11434',
      OLLAMA_ORIGINS: 'http://127.0.0.1:*',
      OLLAMA_MODELS: modelsDir,
    }

    console.log(`[BackendService] Spawning: ${binaryPath} serve`)

    this.process = spawn(binaryPath, ['serve'], {
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
      detached: false,
    })

    this.process.stdout?.on('data', (data: Buffer) => {
      console.log(`[bobe-service] ${data.toString().trimEnd()}`)
    })

    this.process.stderr?.on('data', (data: Buffer) => {
      console.error(`[bobe-service] ${data.toString().trimEnd()}`)
    })

    this.process.on('error', (error) => {
      console.error('[BackendService] Spawn error:', error)
      this.process = null
      if (!this.stopping) {
        this.setState('crashed')
        this.emit('crashed', null)
      }
    })

    this.process.on('exit', (code, signal) => {
      console.log(`[BackendService] Process exited: code=${code}, signal=${signal}`)
      this.process = null

      if (!this.stopping && (this._state === 'starting' || this._state === 'ready')) {
        this.setState('crashed')
        this.emit('crashed', code)
      }
    })
  }

  /** Wait for the health endpoint to respond successfully */
  private async waitForHealth(): Promise<void> {
    let delay = HEALTH_INITIAL_DELAY_MS

    for (let attempt = 0; attempt < MAX_HEALTH_ATTEMPTS; attempt++) {
      await this.sleep(delay)

      if (this.stopping || this._state === 'stopped' || this._state === 'fatal') return
      // If we spawned the process and it died, abort (crash events already emitted)
      if (this.managed && !this.process) return

      try {
        const response = await fetch(HEALTH_URL)
        if (response.ok) {
          console.log('[BackendService] Backend is healthy')
          this.setState('ready')
          this.emit('ready')
          return
        }
      } catch {
        // Service not up yet
      }

      delay = Math.min(delay * 1.5, HEALTH_MAX_DELAY_MS)
    }

    if (!this.stopping) {
      console.error('[BackendService] Backend failed to become healthy')
      this.setState(this.managed ? 'fatal' : 'crashed')
      this.emit(this.managed ? 'fatal' : 'crashed', null)
    }
  }

  private setState(state: BackendServiceState): void {
    this._state = state
    this.emit('state', state)
  }

  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms))
  }
}

export const backendService = new BackendService()
