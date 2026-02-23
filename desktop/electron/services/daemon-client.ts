/**
 * Daemon Client - HTTP + SSE communication with backend daemon
 *
 * Handles all communication with the service:
 * - HTTP requests for commands (capture, message, etc.)
 * - SSE connection for real-time state updates
 * - Auto-reconnection with exponential backoff
 *
 * Domain-specific API methods (entities, tools, settings) are
 * implemented in daemon-api/ modules and delegated to from thin wrappers here.
 *
 * The renderer never talks to the daemon directly - everything routes through here.
 */

import http from 'node:http'
import type {
  StreamBundle,
  HealthResponse,
  StatusResponse,
  CaptureResponse,
  MessageResponse,
  TextDeltaPayload,
  DaemonClientEventMap,
  // Parameter types used by delegate wrappers
  GoalCreateRequest,
  GoalUpdateRequest,
  SoulCreateRequest,
  SoulUpdateRequest,
  UserProfileCreateRequest,
  UserProfileUpdateRequest,
  MCPServerCreateRequest,
  MCPServerUpdateRequest,
  SettingsUpdateRequest,
  MemoryListParams,
  MemoryCreateRequest,
  MemoryUpdateRequest,
} from '../types'

import * as entitiesApi from './daemon-api/entities'
import * as toolsApi from './daemon-api/tools'
import * as settingsApi from './daemon-api/settings'

const DEFAULT_BASE_URL = 'http://localhost:8766'
const MAX_RECONNECT_ATTEMPTS = 10
const INITIAL_RECONNECT_DELAY_MS = 1000
const MAX_RECONNECT_DELAY_MS = 30000

type EventCallback<T> = (data: T) => void

class DaemonClient {
  private baseUrl: string
  private sseRequest: http.ClientRequest | null = null
  private _connected = false
  private reconnectAttempts = 0
  private reconnectTimeout: NodeJS.Timeout | null = null
  private listeners = new Map<string, Set<EventCallback<unknown>>>()

  // Text delta accumulator
  private currentMessage = ''
  private currentMessageId: string | null = null

  // SSE parsing state
  private sseBuffer = ''

  constructor(baseUrl = DEFAULT_BASE_URL) {
    this.baseUrl = baseUrl
  }

  // ===========================================================================
  // CONNECTION STATE
  // ===========================================================================

  get connected(): boolean {
    return this._connected
  }

  getBaseUrl(): string {
    return this.baseUrl
  }

  // ===========================================================================
  // EVENT EMITTER
  // ===========================================================================

  on<K extends keyof DaemonClientEventMap>(
    event: K,
    callback: EventCallback<DaemonClientEventMap[K]>,
  ): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set())
    }
    this.listeners.get(event)!.add(callback as EventCallback<unknown>)

    // Return unsubscribe function
    return () => {
      this.listeners.get(event)?.delete(callback as EventCallback<unknown>)
    }
  }

  private emit<K extends keyof DaemonClientEventMap>(
    event: K,
    data: DaemonClientEventMap[K],
  ): void {
    this.listeners.get(event)?.forEach((callback) => {
      try {
        callback(data)
      } catch (error) {
        console.error(`[DaemonClient] Error in ${event} listener:`, error)
      }
    })
  }

  // ===========================================================================
  // CORE HTTP METHODS
  // ===========================================================================

  async health(): Promise<HealthResponse | null> {
    try {
      const response = await fetch(`${this.baseUrl}/health`)
      if (!response.ok) return null
      return (await response.json()) as HealthResponse
    } catch {
      return null
    }
  }

  async getStatus(): Promise<StatusResponse | null> {
    try {
      const response = await fetch(`${this.baseUrl}/status`)
      if (!response.ok) return null
      return (await response.json()) as StatusResponse
    } catch {
      return null
    }
  }

  async startCapture(): Promise<CaptureResponse> {
    const response = await fetch(`${this.baseUrl}/capture/start`, { method: 'POST' })
    if (!response.ok) throw new Error(`Failed to start capture: ${response.status}`)
    return (await response.json()) as CaptureResponse
  }

  async stopCapture(): Promise<CaptureResponse> {
    const response = await fetch(`${this.baseUrl}/capture/stop`, { method: 'POST' })
    if (!response.ok) throw new Error(`Failed to stop capture: ${response.status}`)
    return (await response.json()) as CaptureResponse
  }

  async sendMessage(content: string): Promise<MessageResponse> {
    const response = await fetch(`${this.baseUrl}/message`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    })
    if (!response.ok) throw new Error(`Failed to send message: ${response.status}`)
    return (await response.json()) as MessageResponse
  }

  // -- Goals (delegates to daemon-api/entities.ts) --------------------------
  async listGoals() {
    return entitiesApi.listGoals(this.baseUrl)
  }
  async getGoal(id: string) {
    return entitiesApi.getGoal(this.baseUrl, id)
  }
  async createGoal(data: GoalCreateRequest) {
    return entitiesApi.createGoal(this.baseUrl, data)
  }
  async updateGoal(id: string, data: GoalUpdateRequest) {
    return entitiesApi.updateGoal(this.baseUrl, id, data)
  }
  async deleteGoal(id: string) {
    return entitiesApi.deleteGoal(this.baseUrl, id)
  }
  async completeGoal(id: string) {
    return entitiesApi.completeGoal(this.baseUrl, id)
  }
  async archiveGoal(id: string) {
    return entitiesApi.archiveGoal(this.baseUrl, id)
  }

  // -- Souls (delegates to daemon-api/entities.ts) --------------------------
  async listSouls() {
    return entitiesApi.listSouls(this.baseUrl)
  }
  async getSoul(id: string) {
    return entitiesApi.getSoul(this.baseUrl, id)
  }
  async createSoul(data: SoulCreateRequest) {
    return entitiesApi.createSoul(this.baseUrl, data)
  }
  async updateSoul(id: string, data: SoulUpdateRequest) {
    return entitiesApi.updateSoul(this.baseUrl, id, data)
  }
  async deleteSoul(id: string) {
    return entitiesApi.deleteSoul(this.baseUrl, id)
  }
  async enableSoul(id: string) {
    return entitiesApi.enableSoul(this.baseUrl, id)
  }
  async disableSoul(id: string) {
    return entitiesApi.disableSoul(this.baseUrl, id)
  }

  // -- User Profiles (delegates to daemon-api/entities.ts) ------------------
  async listUserProfiles() {
    return entitiesApi.listUserProfiles(this.baseUrl)
  }
  async getUserProfile(id: string) {
    return entitiesApi.getUserProfile(this.baseUrl, id)
  }
  async createUserProfile(data: UserProfileCreateRequest) {
    return entitiesApi.createUserProfile(this.baseUrl, data)
  }
  async updateUserProfile(id: string, data: UserProfileUpdateRequest) {
    return entitiesApi.updateUserProfile(this.baseUrl, id, data)
  }
  async deleteUserProfile(id: string) {
    return entitiesApi.deleteUserProfile(this.baseUrl, id)
  }
  async enableUserProfile(id: string) {
    return entitiesApi.enableUserProfile(this.baseUrl, id)
  }
  async disableUserProfile(id: string) {
    return entitiesApi.disableUserProfile(this.baseUrl, id)
  }

  // -- Tools (delegates to daemon-api/tools.ts) -----------------------------
  async listTools() {
    return toolsApi.listTools(this.baseUrl)
  }
  async enableTool(name: string) {
    return toolsApi.enableTool(this.baseUrl, name)
  }
  async disableTool(name: string) {
    return toolsApi.disableTool(this.baseUrl, name)
  }

  // -- MCP Servers (delegates to daemon-api/tools.ts) -----------------------
  async listMCPServers() {
    return toolsApi.listMCPServers(this.baseUrl)
  }
  async listMCPConfigs() {
    return toolsApi.listMCPConfigs(this.baseUrl)
  }
  async createMCPServer(data: MCPServerCreateRequest) {
    return toolsApi.createMCPServer(this.baseUrl, data)
  }
  async updateMCPServer(id: string, data: MCPServerUpdateRequest) {
    return toolsApi.updateMCPServer(this.baseUrl, id, data)
  }
  async deleteMCPServer(name: string) {
    return toolsApi.deleteMCPServer(this.baseUrl, name)
  }
  async reconnectMCPServer(name: string) {
    return toolsApi.reconnectMCPServer(this.baseUrl, name)
  }

  // -- Settings (delegates to daemon-api/settings.ts) -----------------------
  async getSettings() {
    return settingsApi.getSettings(this.baseUrl)
  }
  async updateSettings(data: SettingsUpdateRequest) {
    return settingsApi.updateSettings(this.baseUrl, data)
  }

  // -- Models (delegates to daemon-api/tools.ts) ----------------------------
  async getModels() {
    return toolsApi.getModels(this.baseUrl)
  }
  async getRegistryModels() {
    return toolsApi.getRegistryModels(this.baseUrl)
  }
  async pullModel(modelName: string) {
    return toolsApi.pullModel(this.baseUrl, modelName)
  }
  async deleteModel(modelName: string) {
    return toolsApi.deleteModel(this.baseUrl, modelName)
  }

  // -- Memories (delegates to daemon-api/entities.ts) -----------------------
  async listMemories(params?: MemoryListParams) {
    return entitiesApi.listMemories(this.baseUrl, params)
  }
  async getMemory(id: string) {
    return entitiesApi.getMemory(this.baseUrl, id)
  }
  async createMemory(data: MemoryCreateRequest) {
    return entitiesApi.createMemory(this.baseUrl, data)
  }
  async updateMemory(id: string, data: MemoryUpdateRequest) {
    return entitiesApi.updateMemory(this.baseUrl, id, data)
  }
  async deleteMemory(id: string) {
    return entitiesApi.deleteMemory(this.baseUrl, id)
  }
  async enableMemory(id: string) {
    return entitiesApi.enableMemory(this.baseUrl, id)
  }
  async disableMemory(id: string) {
    return entitiesApi.disableMemory(this.baseUrl, id)
  }

  // ===========================================================================
  // SSE CONNECTION
  // ===========================================================================

  async connect(): Promise<void> {
    const health = await this.health()
    if (!health) {
      console.log('[DaemonClient] Daemon not available, will retry...')
      this.scheduleReconnect()
      return
    }
    this.establishSSE()
  }

  disconnect(): void {
    this.clearReconnectTimeout()
    if (this.sseRequest) {
      this.sseRequest.destroy()
      this.sseRequest = null
    }
    if (this._connected) {
      this._connected = false
      this.emit('disconnected', undefined)
    }
  }

  private establishSSE(): void {
    if (this.sseRequest) {
      this.sseRequest.destroy()
    }

    console.log('[DaemonClient] Establishing SSE connection...')
    const url = new URL(`${this.baseUrl}/events`)

    this.sseRequest = http.request(
      {
        hostname: url.hostname,
        port: url.port || 80,
        path: url.pathname,
        method: 'GET',
        headers: {
          Accept: 'text/event-stream',
          'Cache-Control': 'no-cache',
          Connection: 'keep-alive',
        },
      },
      (response) => {
        if (response.statusCode !== 200) {
          console.error('[DaemonClient] SSE connection failed:', response.statusCode)
          this.scheduleReconnect()
          return
        }

        console.log('[DaemonClient] SSE connected')
        this._connected = true
        this.reconnectAttempts = 0
        this.sseBuffer = ''
        this.emit('connected', undefined)

        response.setEncoding('utf8')

        response.on('data', (chunk: string) => {
          this.handleSSEChunk(chunk)
        })

        response.on('end', () => {
          console.log('[DaemonClient] SSE connection ended')
          if (this._connected) {
            this._connected = false
            this.emit('disconnected', undefined)
          }
          this.scheduleReconnect()
        })

        response.on('error', (error) => {
          console.error('[DaemonClient] SSE response error:', error)
          if (this._connected) {
            this._connected = false
            this.emit('disconnected', undefined)
          }
          this.scheduleReconnect()
        })
      },
    )

    this.sseRequest.on('error', (error) => {
      console.error('[DaemonClient] SSE request error:', error)
      if (this._connected) {
        this._connected = false
        this.emit('disconnected', undefined)
      }
      this.scheduleReconnect()
    })

    this.sseRequest.end()
  }

  // ===========================================================================
  // SSE PARSING
  // ===========================================================================

  private handleSSEChunk(chunk: string): void {
    this.sseBuffer += chunk

    const messages = this.sseBuffer.split('\r\n\r\n')
    this.sseBuffer = messages.pop() || ''

    for (const message of messages) {
      if (!message.trim()) continue
      this.parseSSEMessage(message)
    }
  }

  private parseSSEMessage(message: string): void {
    let data = ''

    for (const line of message.split(/\r?\n/)) {
      if (line.startsWith('data:')) {
        data += line.slice(5).trim()
      }
    }

    if (data) {
      try {
        const bundle = JSON.parse(data) as StreamBundle
        console.log(
          '[DaemonClient] Event:',
          bundle.type,
          bundle.type === 'text_delta' ? (bundle.payload as TextDeltaPayload).delta : '',
        )
        this.processBundle(bundle)
      } catch (error) {
        console.error('[DaemonClient] Failed to parse SSE data:', error, data)
      }
    }
  }

  private processBundle(bundle: StreamBundle): void {
    switch (bundle.type) {
      case 'indicator':
        this.emit('indicator', bundle.payload)
        break
      case 'text_delta':
        this.handleTextDelta(bundle.payload, bundle.message_id)
        break
      case 'error':
        console.error('[DaemonClient] Error event:', bundle.payload)
        this.emit('error', bundle.payload)
        break
      case 'heartbeat':
        this.emit('heartbeat', undefined)
        break
      case 'tool_call':
        this.emit('tool_call', bundle.payload)
        break
      case 'conversation_closed':
        console.log(
          '[DaemonClient] Conversation closed:',
          bundle.payload.conversation_id,
          bundle.payload.reason,
        )
        this.emit('conversation_closed', bundle.payload)
        break
      default: {
        const _exhaustive: string = (bundle as { type: string }).type
        console.warn('[DaemonClient] Unknown bundle type:', _exhaustive)
      }
    }
  }

  // ===========================================================================
  // TEXT DELTA ACCUMULATOR
  // ===========================================================================

  private handleTextDelta(payload: TextDeltaPayload, messageId: string): void {
    if (this.currentMessageId !== messageId) {
      this.currentMessage = ''
      this.currentMessageId = messageId
    }

    this.currentMessage += payload.delta
    this.emit('text_delta', { ...payload, message_id: messageId })

    if (payload.done) {
      this.emit('message_complete', {
        message_id: messageId,
        content: this.currentMessage,
      })
      this.currentMessage = ''
      this.currentMessageId = null
    }
  }

  // ===========================================================================
  // RECONNECTION
  // ===========================================================================

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
      console.error('[DaemonClient] Max reconnect attempts reached, giving up')
      return
    }

    this.reconnectAttempts++
    const delay = Math.min(
      INITIAL_RECONNECT_DELAY_MS * Math.pow(2, this.reconnectAttempts - 1),
      MAX_RECONNECT_DELAY_MS,
    )

    console.log(
      `[DaemonClient] Scheduling reconnect attempt ${this.reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS} in ${delay}ms`,
    )

    this.emit('reconnecting', {
      attempt: this.reconnectAttempts,
      maxAttempts: MAX_RECONNECT_ATTEMPTS,
    })

    this.reconnectTimeout = setTimeout(async () => {
      const health = await this.health()
      if (health) {
        this.establishSSE()
      } else {
        this.scheduleReconnect()
      }
    }, delay)
  }

  private clearReconnectTimeout(): void {
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout)
      this.reconnectTimeout = null
    }
  }

  // ===========================================================================
  // ACCESSORS FOR STREAMING STATE
  // ===========================================================================

  getCurrentMessage(): string {
    return this.currentMessage
  }

  getCurrentMessageId(): string | null {
    return this.currentMessageId
  }
}

// Singleton instance
export const daemonClient = new DaemonClient()
