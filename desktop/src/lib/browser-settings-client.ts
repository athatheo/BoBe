/**
 * Browser Settings Client
 *
 * Provides window.goals, window.souls, window.userProfiles, window.tools,
 * window.mcpServers, and window.settings compatible APIs that talk directly
 * to the Python daemon via HTTP. Used when running in a browser without Electron.
 */

import type {
  Goal,
  GoalListResponse,
  GoalCreateRequest,
  GoalUpdateRequest,
  GoalActionResponse,
  Soul,
  SoulListResponse,
  SoulCreateRequest,
  SoulUpdateRequest,
  SoulActionResponse,
  UserProfile,
  UserProfileListResponse,
  UserProfileCreateRequest,
  UserProfileUpdateRequest,
  UserProfileActionResponse,
  ToolListResponse,
  ToolUpdateResponse,
  MCPServerListResponse,
  MCPConfigListResponse,
  MCPServerCreateRequest,
  MCPServerCreateResponse,
  MCPServerUpdateRequest,
  MCPServerUpdateResponse,
  MCPServerDeleteResponse,
  MCPServerReconnectResponse,
  Memory,
  MemoryListResponse,
  MemoryListParams,
  MemoryCreateRequest,
  MemoryUpdateRequest,
  MemoryActionResponse,
} from '@/types/api'
import type {
  GoalsAPI,
  SoulsAPI,
  UserProfilesAPI,
  ToolsAPI,
  MCPServersAPI,
  MemoriesAPI,
} from '@/types/ipc'

const DAEMON_URL = 'http://localhost:8766'
const FETCH_TIMEOUT = 10_000 // 10s timeout on all daemon requests

/** Fetch with timeout to prevent hanging requests */
function daemonFetch(url: string, init?: RequestInit): Promise<Response> {
  return fetch(url, { signal: AbortSignal.timeout(FETCH_TIMEOUT), ...init })
}

// =============================================================================
// GOALS CLIENT
// =============================================================================

class BrowserGoalsClient implements GoalsAPI {
  async list(): Promise<GoalListResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/goals`)
    if (!response.ok) throw new Error(`Failed to list goals: ${response.status}`)
    return response.json()
  }

  async get(id: string): Promise<Goal> {
    const response = await daemonFetch(`${DAEMON_URL}/goals/${id}`)
    if (!response.ok) throw new Error(`Failed to get goal: ${response.status}`)
    return response.json()
  }

  async create(data: GoalCreateRequest): Promise<Goal> {
    const response = await daemonFetch(`${DAEMON_URL}/goals`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) {
      const errorBody = await response.text()
      throw new Error(`Failed to create goal: ${response.status} - ${errorBody}`)
    }
    return response.json()
  }

  async update(id: string, data: GoalUpdateRequest): Promise<Goal> {
    const response = await daemonFetch(`${DAEMON_URL}/goals/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) throw new Error(`Failed to update goal: ${response.status}`)
    return response.json()
  }

  async delete(id: string): Promise<GoalActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/goals/${id}`, { method: 'DELETE' })
    if (!response.ok) throw new Error(`Failed to delete goal: ${response.status}`)
    return response.json()
  }

  async complete(id: string): Promise<GoalActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/goals/${id}/complete`, { method: 'POST' })
    if (!response.ok) throw new Error(`Failed to complete goal: ${response.status}`)
    return response.json()
  }

  async archive(id: string): Promise<GoalActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/goals/${id}/archive`, { method: 'POST' })
    if (!response.ok) throw new Error(`Failed to archive goal: ${response.status}`)
    return response.json()
  }
}

// =============================================================================
// SOULS CLIENT
// =============================================================================

class BrowserSoulsClient implements SoulsAPI {
  async list(): Promise<SoulListResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/souls`)
    if (!response.ok) throw new Error(`Failed to list souls: ${response.status}`)
    return response.json()
  }

  async get(id: string): Promise<Soul> {
    const response = await daemonFetch(`${DAEMON_URL}/souls/${id}`)
    if (!response.ok) throw new Error(`Failed to get soul: ${response.status}`)
    return response.json()
  }

  async create(data: SoulCreateRequest): Promise<Soul> {
    const response = await daemonFetch(`${DAEMON_URL}/souls`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) {
      const errorBody = await response.text()
      throw new Error(`Failed to create soul: ${response.status} - ${errorBody}`)
    }
    return response.json()
  }

  async update(id: string, data: SoulUpdateRequest): Promise<Soul> {
    const response = await daemonFetch(`${DAEMON_URL}/souls/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) throw new Error(`Failed to update soul: ${response.status}`)
    return response.json()
  }

  async delete(id: string): Promise<SoulActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/souls/${id}`, { method: 'DELETE' })
    if (!response.ok) throw new Error(`Failed to delete soul: ${response.status}`)
    return response.json()
  }

  async enable(id: string): Promise<SoulActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/souls/${id}/enable`, { method: 'POST' })
    if (!response.ok) throw new Error(`Failed to enable soul: ${response.status}`)
    return response.json()
  }

  async disable(id: string): Promise<SoulActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/souls/${id}/disable`, { method: 'POST' })
    if (!response.ok) throw new Error(`Failed to disable soul: ${response.status}`)
    return response.json()
  }
}

// =============================================================================
// USER PROFILES CLIENT
// =============================================================================

class BrowserUserProfilesClient implements UserProfilesAPI {
  async list(): Promise<UserProfileListResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/user-profiles`)
    if (!response.ok) throw new Error(`Failed to list user profiles: ${response.status}`)
    return response.json()
  }

  async get(id: string): Promise<UserProfile> {
    const response = await daemonFetch(`${DAEMON_URL}/user-profiles/${id}`)
    if (!response.ok) throw new Error(`Failed to get user profile: ${response.status}`)
    return response.json()
  }

  async create(data: UserProfileCreateRequest): Promise<UserProfile> {
    const response = await daemonFetch(`${DAEMON_URL}/user-profiles`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) {
      const errorBody = await response.text()
      throw new Error(`Failed to create user profile: ${response.status} - ${errorBody}`)
    }
    return response.json()
  }

  async update(id: string, data: UserProfileUpdateRequest): Promise<UserProfile> {
    const response = await daemonFetch(`${DAEMON_URL}/user-profiles/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) throw new Error(`Failed to update user profile: ${response.status}`)
    return response.json()
  }

  async delete(id: string): Promise<UserProfileActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/user-profiles/${id}`, { method: 'DELETE' })
    if (!response.ok) throw new Error(`Failed to delete user profile: ${response.status}`)
    return response.json()
  }

  async enable(id: string): Promise<UserProfileActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/user-profiles/${id}/enable`, {
      method: 'POST',
    })
    if (!response.ok) throw new Error(`Failed to enable user profile: ${response.status}`)
    return response.json()
  }

  async disable(id: string): Promise<UserProfileActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/user-profiles/${id}/disable`, {
      method: 'POST',
    })
    if (!response.ok) throw new Error(`Failed to disable user profile: ${response.status}`)
    return response.json()
  }
}

// =============================================================================
// TOOLS CLIENT
// =============================================================================

class BrowserToolsClient implements ToolsAPI {
  async list(): Promise<ToolListResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/tools`)
    if (!response.ok) throw new Error(`Failed to list tools: ${response.status}`)
    return response.json()
  }

  async enable(name: string): Promise<ToolUpdateResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/tools/${encodeURIComponent(name)}/enable`, {
      method: 'POST',
    })
    if (!response.ok) throw new Error(`Failed to enable tool: ${response.status}`)
    return response.json()
  }

  async disable(name: string): Promise<ToolUpdateResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/tools/${encodeURIComponent(name)}/disable`, {
      method: 'POST',
    })
    if (!response.ok) throw new Error(`Failed to disable tool: ${response.status}`)
    return response.json()
  }
}

// =============================================================================
// MCP SERVERS CLIENT
// =============================================================================

class BrowserMCPServersClient implements MCPServersAPI {
  async list(): Promise<MCPServerListResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/tools/mcp`)
    if (!response.ok) throw new Error(`Failed to list MCP servers: ${response.status}`)
    return response.json()
  }

  async listConfigs(): Promise<MCPConfigListResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/mcp-configs`)
    if (!response.ok) throw new Error(`Failed to list MCP configs: ${response.status}`)
    return response.json()
  }

  async create(data: MCPServerCreateRequest): Promise<MCPServerCreateResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/tools/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) {
      const errorBody = await response.text()
      throw new Error(`Failed to create MCP server: ${response.status} - ${errorBody}`)
    }
    return response.json()
  }

  async update(id: string, data: MCPServerUpdateRequest): Promise<MCPServerUpdateResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/mcp-configs/${encodeURIComponent(id)}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) {
      const errorBody = await response.text()
      throw new Error(`Failed to update MCP server: ${response.status} - ${errorBody}`)
    }
    return response.json()
  }

  async delete(name: string): Promise<MCPServerDeleteResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/tools/mcp/${encodeURIComponent(name)}`, {
      method: 'DELETE',
    })
    if (!response.ok) throw new Error(`Failed to delete MCP server: ${response.status}`)
    return response.json()
  }

  async reconnect(name: string): Promise<MCPServerReconnectResponse> {
    const response = await daemonFetch(
      `${DAEMON_URL}/tools/mcp/${encodeURIComponent(name)}/reconnect`,
      {
        method: 'POST',
      },
    )
    if (!response.ok) throw new Error(`Failed to reconnect MCP server: ${response.status}`)
    return response.json()
  }
}

// =============================================================================
// MEMORIES CLIENT
// =============================================================================

class BrowserMemoriesClient implements MemoriesAPI {
  async list(params?: MemoryListParams): Promise<MemoryListResponse> {
    const url = new URL(`${DAEMON_URL}/memories`)
    if (params) {
      if (params.memory_type) url.searchParams.set('memory_type', params.memory_type)
      if (params.category) url.searchParams.set('category', params.category)
      if (params.source) url.searchParams.set('source', params.source)
      if (params.enabled_only !== undefined)
        url.searchParams.set('enabled_only', String(params.enabled_only))
      if (params.limit !== undefined) url.searchParams.set('limit', String(params.limit))
      if (params.offset !== undefined) url.searchParams.set('offset', String(params.offset))
    }
    const response = await fetch(url.toString())
    if (!response.ok) throw new Error(`Failed to list memories: ${response.status}`)
    return response.json()
  }

  async get(id: string): Promise<Memory> {
    const response = await daemonFetch(`${DAEMON_URL}/memories/${id}`)
    if (!response.ok) throw new Error(`Failed to get memory: ${response.status}`)
    return response.json()
  }

  async create(data: MemoryCreateRequest): Promise<Memory> {
    const response = await daemonFetch(`${DAEMON_URL}/memories`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) {
      const errorBody = await response.text()
      throw new Error(`Failed to create memory: ${response.status} - ${errorBody}`)
    }
    return response.json()
  }

  async update(id: string, data: MemoryUpdateRequest): Promise<Memory> {
    const response = await daemonFetch(`${DAEMON_URL}/memories/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
    if (!response.ok) throw new Error(`Failed to update memory: ${response.status}`)
    return response.json()
  }

  async delete(id: string): Promise<MemoryActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/memories/${id}`, { method: 'DELETE' })
    if (!response.ok) throw new Error(`Failed to delete memory: ${response.status}`)
    return response.json()
  }

  async enable(id: string): Promise<MemoryActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/memories/${id}/enable`, { method: 'POST' })
    if (!response.ok) throw new Error(`Failed to enable memory: ${response.status}`)
    return response.json()
  }

  async disable(id: string): Promise<MemoryActionResponse> {
    const response = await daemonFetch(`${DAEMON_URL}/memories/${id}/disable`, { method: 'POST' })
    if (!response.ok) throw new Error(`Failed to disable memory: ${response.status}`)
    return response.json()
  }
}

// =============================================================================
// SINGLETON INSTANCES
// =============================================================================

let goalsClient: BrowserGoalsClient | null = null
let soulsClient: BrowserSoulsClient | null = null
let userProfilesClient: BrowserUserProfilesClient | null = null
let toolsClient: BrowserToolsClient | null = null
let mcpServersClient: BrowserMCPServersClient | null = null
let memoriesClient: BrowserMemoriesClient | null = null

// =============================================================================
// CLIENT GETTERS
// =============================================================================

export function getBrowserGoalsClient(): GoalsAPI {
  if (!goalsClient) goalsClient = new BrowserGoalsClient()
  return goalsClient
}

export function getBrowserSoulsClient(): SoulsAPI {
  if (!soulsClient) soulsClient = new BrowserSoulsClient()
  return soulsClient
}

export function getBrowserUserProfilesClient(): UserProfilesAPI {
  if (!userProfilesClient) userProfilesClient = new BrowserUserProfilesClient()
  return userProfilesClient
}

export function getBrowserToolsClient(): ToolsAPI {
  if (!toolsClient) toolsClient = new BrowserToolsClient()
  return toolsClient
}

export function getBrowserMCPServersClient(): MCPServersAPI {
  if (!mcpServersClient) mcpServersClient = new BrowserMCPServersClient()
  return mcpServersClient
}

export function getBrowserMemoriesClient(): MemoriesAPI {
  if (!memoriesClient) memoriesClient = new BrowserMemoriesClient()
  return memoriesClient
}

// =============================================================================
// ENVIRONMENT DETECTION
// =============================================================================

function hasElectron(api: string): boolean {
  return (
    typeof window !== 'undefined' &&
    api in window &&
    (window as unknown as Record<string, unknown>)[api] !== undefined
  )
}

export function getGoalsClient(): GoalsAPI {
  return hasElectron('goals') ? window.goals : getBrowserGoalsClient()
}

export function getSoulsClient(): SoulsAPI {
  return hasElectron('souls') ? window.souls : getBrowserSoulsClient()
}

export function getUserProfilesClient(): UserProfilesAPI {
  return hasElectron('userProfiles') ? window.userProfiles : getBrowserUserProfilesClient()
}

export function getToolsClient(): ToolsAPI {
  return hasElectron('tools') ? window.tools : getBrowserToolsClient()
}

export function getMCPServersClient(): MCPServersAPI {
  return hasElectron('mcpServers') ? window.mcpServers : getBrowserMCPServersClient()
}

export function getMemoriesClient(): MemoriesAPI {
  return hasElectron('memories') ? window.memories : getBrowserMemoriesClient()
}
