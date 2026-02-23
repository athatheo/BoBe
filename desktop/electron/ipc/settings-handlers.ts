/**
 * Settings IPC handlers
 *
 * Handles all settings-related IPC communication for:
 * - Goals
 * - Souls
 * - User Profiles
 * - Tools
 * - MCP Servers
 * - Settings
 *
 * Routes through daemon client to Python backend.
 */

import { ipcMain, dialog } from 'electron'
import { IPC_CHANNELS } from '../types'
import { daemonClient } from '../services'
import type {
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
  MemoryCategory,
  MemoryType,
  MemorySource,
  GoalPriority,
  GoalStatus,
} from '../types/daemon'

// =============================================================================
// VALIDATION HELPERS
// =============================================================================

// Reserved names that could conflict with system paths
const RESERVED_NAMES = ['con', 'prn', 'aux', 'nul', 'com1', 'lpt1', 'default', 'system']

const VALID_GOAL_PRIORITIES: GoalPriority[] = ['high', 'medium', 'low']
const VALID_GOAL_STATUSES: GoalStatus[] = ['active', 'completed', 'archived']
const VALID_MEMORY_TYPES: MemoryType[] = ['short_term', 'long_term', 'explicit']
const VALID_MEMORY_CATEGORIES: MemoryCategory[] = [
  'preference',
  'pattern',
  'fact',
  'interest',
  'general',
  'observation',
]
const VALID_MEMORY_SOURCES: MemorySource[] = ['observation', 'conversation', 'user', 'visual_diary']

function isValidId(id: unknown): id is string {
  return typeof id === 'string' && id.length > 0
}

function isValidString(value: unknown, maxLength = 500): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= maxLength
}

function isValidName(name: unknown): name is string {
  if (typeof name !== 'string' || name.length === 0 || name.length > 255) {
    return false
  }
  // Must be lowercase alphanumeric with hyphens/underscores, starting with letter
  if (!/^[a-z][a-z0-9_-]*$/.test(name)) {
    return false
  }
  // No reserved names
  if (RESERVED_NAMES.includes(name.toLowerCase())) {
    return false
  }
  return true
}

function validateArgsArray(args: unknown): args is string[] {
  if (!Array.isArray(args)) {
    return false
  }
  if (args.length > 100) {
    throw new Error('Args array too large (max 100 items)')
  }
  for (const arg of args) {
    if (typeof arg !== 'string') {
      throw new Error('Each arg must be a string')
    }
    if (arg.length > 1000) {
      throw new Error('Arg too long (max 1000 chars each)')
    }
  }
  return true
}

function validateEnvObject(env: unknown): env is Record<string, string> {
  if (typeof env !== 'object' || env === null || Array.isArray(env)) {
    return false
  }
  const entries = Object.entries(env)
  if (entries.length > 50) {
    throw new Error('Too many env variables (max 50)')
  }
  for (const [key, value] of entries) {
    if (typeof key !== 'string' || key.length === 0 || key.length > 255) {
      throw new Error('Env key must be 1-255 characters')
    }
    if (typeof value !== 'string' || value.length > 5000) {
      throw new Error('Env value must be a string (max 5000 chars)')
    }
  }
  return true
}

// =============================================================================
// SETUP ALL HANDLERS
// =============================================================================

/**
 * Setup all settings-related IPC handlers
 */
export function setupSettingsIpcHandlers(): void {
  // -------------------------------------------------------------------------
  // Goals API
  // -------------------------------------------------------------------------

  ipcMain.handle(IPC_CHANNELS.GOALS_LIST, async () => {
    return daemonClient.listGoals()
  })

  ipcMain.handle(IPC_CHANNELS.GOALS_GET, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid goal ID')
    }
    return daemonClient.getGoal(id)
  })

  ipcMain.handle(IPC_CHANNELS.GOALS_CREATE, async (_e, data: GoalCreateRequest) => {
    const { content, priority, enabled } = data || {}

    if (!isValidString(content, 500)) {
      throw new Error('Invalid goal content: must be 1-500 characters')
    }
    if (priority !== undefined && !VALID_GOAL_PRIORITIES.includes(priority)) {
      throw new Error(`Invalid priority: ${priority}`)
    }

    return daemonClient.createGoal({
      content,
      priority: priority ?? 'medium',
      enabled: enabled ?? true,
    })
  })

  ipcMain.handle(IPC_CHANNELS.GOALS_UPDATE, async (_e, id: string, data: GoalUpdateRequest) => {
    if (!isValidId(id)) {
      throw new Error('Invalid goal ID')
    }

    const updateData: GoalUpdateRequest = {}
    if (data.content !== undefined) {
      if (!isValidString(data.content, 500)) {
        throw new Error('Invalid goal content')
      }
      updateData.content = data.content
    }
    if (data.status !== undefined) {
      if (!VALID_GOAL_STATUSES.includes(data.status)) {
        throw new Error(`Invalid status: ${data.status}`)
      }
      updateData.status = data.status
    }
    if (data.priority !== undefined) {
      if (!VALID_GOAL_PRIORITIES.includes(data.priority)) {
        throw new Error(`Invalid priority: ${data.priority}`)
      }
      updateData.priority = data.priority
    }
    if (data.enabled !== undefined) {
      if (typeof data.enabled !== 'boolean') {
        throw new Error('Enabled must be a boolean')
      }
      updateData.enabled = data.enabled
    }

    return daemonClient.updateGoal(id, updateData)
  })

  ipcMain.handle(IPC_CHANNELS.GOALS_DELETE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid goal ID')
    }
    return daemonClient.deleteGoal(id)
  })

  ipcMain.handle(IPC_CHANNELS.GOALS_COMPLETE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid goal ID')
    }
    return daemonClient.completeGoal(id)
  })

  ipcMain.handle(IPC_CHANNELS.GOALS_ARCHIVE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid goal ID')
    }
    return daemonClient.archiveGoal(id)
  })

  // -------------------------------------------------------------------------
  // Souls API
  // -------------------------------------------------------------------------

  ipcMain.handle(IPC_CHANNELS.SOULS_LIST, async () => {
    return daemonClient.listSouls()
  })

  ipcMain.handle(IPC_CHANNELS.SOULS_GET, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid soul ID')
    }
    return daemonClient.getSoul(id)
  })

  ipcMain.handle(IPC_CHANNELS.SOULS_CREATE, async (_e, data: SoulCreateRequest) => {
    const { name, content, enabled } = data || {}

    if (!isValidName(name)) {
      throw new Error('Invalid soul name: must be lowercase alphanumeric starting with letter')
    }
    if (typeof content !== 'string' || content.length < 10) {
      throw new Error('Invalid content: must be at least 10 characters (markdown)')
    }

    return daemonClient.createSoul({
      name,
      content,
      enabled: enabled ?? true,
    })
  })

  ipcMain.handle(IPC_CHANNELS.SOULS_UPDATE, async (_e, id: string, data: SoulUpdateRequest) => {
    if (!isValidId(id)) {
      throw new Error('Invalid soul ID')
    }

    const updateData: SoulUpdateRequest = {}
    if (data.content !== undefined) {
      if (typeof data.content !== 'string') {
        throw new Error('Content must be a string')
      }
      updateData.content = data.content
    }
    if (data.enabled !== undefined) {
      if (typeof data.enabled !== 'boolean') {
        throw new Error('Enabled must be a boolean')
      }
      updateData.enabled = data.enabled
    }

    return daemonClient.updateSoul(id, updateData)
  })

  ipcMain.handle(IPC_CHANNELS.SOULS_DELETE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid soul ID')
    }
    return daemonClient.deleteSoul(id)
  })

  ipcMain.handle(IPC_CHANNELS.SOULS_ENABLE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid soul ID')
    }
    return daemonClient.enableSoul(id)
  })

  ipcMain.handle(IPC_CHANNELS.SOULS_DISABLE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid soul ID')
    }
    return daemonClient.disableSoul(id)
  })

  // -------------------------------------------------------------------------
  // User Profiles API
  // -------------------------------------------------------------------------

  ipcMain.handle(IPC_CHANNELS.USER_PROFILES_LIST, async () => {
    return daemonClient.listUserProfiles()
  })

  ipcMain.handle(IPC_CHANNELS.USER_PROFILES_GET, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid user profile ID')
    }
    return daemonClient.getUserProfile(id)
  })

  ipcMain.handle(IPC_CHANNELS.USER_PROFILES_CREATE, async (_e, data: UserProfileCreateRequest) => {
    const { name, content, enabled } = data || {}

    if (!isValidName(name)) {
      throw new Error('Invalid profile name: must be lowercase alphanumeric starting with letter')
    }
    if (typeof content !== 'string' || content.length < 10) {
      throw new Error('Invalid content: must be at least 10 characters')
    }

    return daemonClient.createUserProfile({
      name,
      content,
      enabled: enabled ?? true,
    })
  })

  ipcMain.handle(
    IPC_CHANNELS.USER_PROFILES_UPDATE,
    async (_e, id: string, data: UserProfileUpdateRequest) => {
      if (!isValidId(id)) {
        throw new Error('Invalid user profile ID')
      }

      const updateData: UserProfileUpdateRequest = {}
      if (data.content !== undefined) {
        if (typeof data.content !== 'string') {
          throw new Error('Content must be a string')
        }
        updateData.content = data.content
      }
      if (data.enabled !== undefined) {
        if (typeof data.enabled !== 'boolean') {
          throw new Error('Enabled must be a boolean')
        }
        updateData.enabled = data.enabled
      }

      return daemonClient.updateUserProfile(id, updateData)
    },
  )

  ipcMain.handle(IPC_CHANNELS.USER_PROFILES_DELETE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid user profile ID')
    }
    return daemonClient.deleteUserProfile(id)
  })

  ipcMain.handle(IPC_CHANNELS.USER_PROFILES_ENABLE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid user profile ID')
    }
    return daemonClient.enableUserProfile(id)
  })

  ipcMain.handle(IPC_CHANNELS.USER_PROFILES_DISABLE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid user profile ID')
    }
    return daemonClient.disableUserProfile(id)
  })

  // -------------------------------------------------------------------------
  // Tools API
  // -------------------------------------------------------------------------

  ipcMain.handle(IPC_CHANNELS.TOOLS_LIST, async () => {
    return daemonClient.listTools()
  })

  ipcMain.handle(IPC_CHANNELS.TOOLS_ENABLE, async (_e, name: string) => {
    if (!isValidString(name, 255)) {
      throw new Error('Invalid tool name')
    }
    return daemonClient.enableTool(name)
  })

  ipcMain.handle(IPC_CHANNELS.TOOLS_DISABLE, async (_e, name: string) => {
    if (!isValidString(name, 255)) {
      throw new Error('Invalid tool name')
    }
    return daemonClient.disableTool(name)
  })

  // -------------------------------------------------------------------------
  // MCP Servers API
  // -------------------------------------------------------------------------

  ipcMain.handle(IPC_CHANNELS.MCP_SERVERS_LIST, async () => {
    return daemonClient.listMCPServers()
  })

  ipcMain.handle(IPC_CHANNELS.MCP_SERVERS_LIST_CONFIGS, async () => {
    return daemonClient.listMCPConfigs()
  })

  ipcMain.handle(IPC_CHANNELS.MCP_SERVERS_CREATE, async (_e, data: MCPServerCreateRequest) => {
    const { name, command, args, env, enabled } = data || {}

    if (!isValidName(name)) {
      throw new Error('Invalid server name: must be lowercase alphanumeric starting with letter')
    }
    if (!isValidString(command, 500)) {
      throw new Error('Invalid command: must be 1-500 characters')
    }
    if (args !== undefined) {
      validateArgsArray(args)
    }
    if (env !== undefined) {
      validateEnvObject(env)
    }

    return daemonClient.createMCPServer({
      name,
      command,
      args,
      env,
      enabled: enabled ?? true,
    })
  })

  ipcMain.handle(
    IPC_CHANNELS.MCP_SERVERS_UPDATE,
    async (_e, id: string, data: MCPServerUpdateRequest) => {
      if (!isValidString(id, 255)) {
        throw new Error('Invalid server config id')
      }
      const { excluded_tools } = data || {}
      if (excluded_tools !== undefined) {
        if (
          !Array.isArray(excluded_tools) ||
          !excluded_tools.every((t) => typeof t === 'string' && t.length <= 255)
        ) {
          throw new Error('Invalid excluded_tools: must be an array of strings')
        }
      }
      return daemonClient.updateMCPServer(id, { excluded_tools })
    },
  )

  ipcMain.handle(IPC_CHANNELS.MCP_SERVERS_DELETE, async (_e, name: string) => {
    if (!isValidString(name, 255)) {
      throw new Error('Invalid server name')
    }
    return daemonClient.deleteMCPServer(name)
  })

  ipcMain.handle(IPC_CHANNELS.MCP_SERVERS_RECONNECT, async (_e, name: string) => {
    if (!isValidString(name, 255)) {
      throw new Error('Invalid server name')
    }
    return daemonClient.reconnectMCPServer(name)
  })

  // -------------------------------------------------------------------------
  // Settings API
  // -------------------------------------------------------------------------

  ipcMain.handle(IPC_CHANNELS.SETTINGS_GET, async () => {
    return daemonClient.getSettings()
  })

  ipcMain.handle(IPC_CHANNELS.SETTINGS_UPDATE, async (_e, data: SettingsUpdateRequest) => {
    const updateData: SettingsUpdateRequest = {}

    if (data.capture_enabled !== undefined) {
      if (typeof data.capture_enabled !== 'boolean') {
        throw new Error('capture_enabled must be a boolean')
      }
      updateData.capture_enabled = data.capture_enabled
    }
    if (data.capture_interval_seconds !== undefined) {
      if (typeof data.capture_interval_seconds !== 'number' || data.capture_interval_seconds <= 0) {
        throw new Error('capture_interval_seconds must be a positive number')
      }
      updateData.capture_interval_seconds = data.capture_interval_seconds
    }
    if (data.checkin_enabled !== undefined) {
      if (typeof data.checkin_enabled !== 'boolean') {
        throw new Error('checkin_enabled must be a boolean')
      }
      updateData.checkin_enabled = data.checkin_enabled
    }
    if (data.learning_enabled !== undefined) {
      if (typeof data.learning_enabled !== 'boolean') {
        throw new Error('learning_enabled must be a boolean')
      }
      updateData.learning_enabled = data.learning_enabled
    }
    if (data.tools_enabled !== undefined) {
      if (typeof data.tools_enabled !== 'boolean') {
        throw new Error('tools_enabled must be a boolean')
      }
      updateData.tools_enabled = data.tools_enabled
    }
    if (data.mcp_enabled !== undefined) {
      if (typeof data.mcp_enabled !== 'boolean') {
        throw new Error('mcp_enabled must be a boolean')
      }
      updateData.mcp_enabled = data.mcp_enabled
    }

    // New dynamic fields — pass through with basic type validation
    const numericFields = [
      'capture_interval_seconds',
      'checkin_jitter_minutes',
      'learning_interval_minutes',
      'conversation_inactivity_timeout_seconds',
      'conversation_auto_close_minutes',
      'tools_max_iterations',
      'memory_short_term_retention_days',
      'memory_long_term_retention_days',
    ] as const
    for (const field of numericFields) {
      if (data[field] !== undefined) {
        if (typeof data[field] !== 'number' || isNaN(data[field]!) || data[field]! <= 0) {
          throw new Error(`${field} must be a positive number`)
        }
        updateData[field] = data[field]
      }
    }

    const booleanFields = ['conversation_summary_enabled'] as const
    for (const field of booleanFields) {
      if (data[field] !== undefined) {
        if (typeof data[field] !== 'boolean') {
          throw new Error(`${field} must be a boolean`)
        }
        updateData[field] = data[field]
      }
    }

    const floatFields = [
      'goal_check_interval_seconds',
      'similarity_deduplication_threshold',
      'similarity_search_recall_threshold',
      'similarity_clustering_threshold',
    ] as const
    for (const field of floatFields) {
      if (data[field] !== undefined) {
        if (typeof data[field] !== 'number' || isNaN(data[field]!)) {
          throw new Error(`${field} must be a number`)
        }
        updateData[field] = data[field]
      }
    }

    // String fields (model names)
    if (data.ollama_model !== undefined) {
      if (typeof data.ollama_model !== 'string' || data.ollama_model.length === 0) {
        throw new Error('ollama_model must be a non-empty string')
      }
      updateData.ollama_model = data.ollama_model
    }
    if (data.openai_model !== undefined) {
      if (typeof data.openai_model !== 'string' || data.openai_model.length === 0) {
        throw new Error('openai_model must be a non-empty string')
      }
      updateData.openai_model = data.openai_model
    }

    // Array fields
    if (data.checkin_times !== undefined) {
      if (!Array.isArray(data.checkin_times)) {
        throw new Error('checkin_times must be an array of strings')
      }
      updateData.checkin_times = data.checkin_times
    }

    // String fields (projects directory)
    if (data.projects_directory !== undefined) {
      if (typeof data.projects_directory !== 'string') {
        throw new Error('projects_directory must be a string')
      }
      if (data.projects_directory.length > 1024) {
        throw new Error('projects_directory path too long')
      }
      updateData.projects_directory = data.projects_directory
    }

    const result = await daemonClient.updateSettings(updateData)

    return result
  })

  ipcMain.handle(IPC_CHANNELS.SETTINGS_SELECT_DIRECTORY, async () => {
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory', 'createDirectory'],
      title: 'Select Projects Directory',
    })
    if (result.canceled || result.filePaths.length === 0) return null
    return result.filePaths[0]
  })

  // -------------------------------------------------------------------------
  // Models API
  // -------------------------------------------------------------------------

  ipcMain.handle(IPC_CHANNELS.MODELS_LIST, async () => {
    return daemonClient.getModels()
  })

  ipcMain.handle(IPC_CHANNELS.MODELS_REGISTRY, async () => {
    return daemonClient.getRegistryModels()
  })

  ipcMain.handle(IPC_CHANNELS.MODELS_PULL, async (_e, modelName: string) => {
    if (typeof modelName !== 'string' || modelName.length === 0 || modelName.length > 200) {
      throw new Error('modelName must be a non-empty string (max 200 chars)')
    }
    return daemonClient.pullModel(modelName)
  })

  ipcMain.handle(IPC_CHANNELS.MODELS_DELETE, async (_e, modelName: string) => {
    if (typeof modelName !== 'string' || modelName.length === 0) {
      throw new Error('modelName must be a non-empty string')
    }
    return daemonClient.deleteModel(modelName)
  })

  // -------------------------------------------------------------------------
  // Memories API
  // -------------------------------------------------------------------------

  ipcMain.handle(IPC_CHANNELS.MEMORIES_LIST, async (_e, params?: MemoryListParams) => {
    // Validate optional params if provided
    if (params) {
      if (params.memory_type !== undefined && !VALID_MEMORY_TYPES.includes(params.memory_type)) {
        throw new Error(`Invalid memory_type: ${params.memory_type}`)
      }
      if (params.category !== undefined && !VALID_MEMORY_CATEGORIES.includes(params.category)) {
        throw new Error(`Invalid category: ${params.category}`)
      }
      if (params.source !== undefined && !VALID_MEMORY_SOURCES.includes(params.source)) {
        throw new Error(`Invalid source: ${params.source}`)
      }
      if (
        params.limit !== undefined &&
        (typeof params.limit !== 'number' || params.limit < 1 || params.limit > 500)
      ) {
        throw new Error('limit must be between 1 and 500')
      }
      if (params.offset !== undefined && (typeof params.offset !== 'number' || params.offset < 0)) {
        throw new Error('offset must be a non-negative number')
      }
    }
    return daemonClient.listMemories(params)
  })

  ipcMain.handle(IPC_CHANNELS.MEMORIES_GET, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid memory ID')
    }
    return daemonClient.getMemory(id)
  })

  ipcMain.handle(IPC_CHANNELS.MEMORIES_CREATE, async (_e, data: MemoryCreateRequest) => {
    const { content, category, memory_type } = data || {}

    if (!isValidString(content, 2000)) {
      throw new Error('Invalid memory content: must be 1-2000 characters')
    }
    if (category !== undefined && !VALID_MEMORY_CATEGORIES.includes(category)) {
      throw new Error(`Invalid category: ${category}`)
    }
    if (memory_type !== undefined && !VALID_MEMORY_TYPES.includes(memory_type)) {
      throw new Error(`Invalid memory_type: ${memory_type}`)
    }

    return daemonClient.createMemory({
      content,
      category: category ?? 'general',
      memory_type: memory_type ?? 'explicit',
    })
  })

  ipcMain.handle(
    IPC_CHANNELS.MEMORIES_UPDATE,
    async (_e, id: string, data: MemoryUpdateRequest) => {
      if (!isValidId(id)) {
        throw new Error('Invalid memory ID')
      }

      const updateData: MemoryUpdateRequest = {}
      if (data.content !== undefined) {
        if (!isValidString(data.content, 2000)) {
          throw new Error('Invalid content: must be 1-2000 characters')
        }
        updateData.content = data.content
      }
      if (data.category !== undefined) {
        if (!VALID_MEMORY_CATEGORIES.includes(data.category)) {
          throw new Error(`Invalid category: ${data.category}`)
        }
        updateData.category = data.category
      }
      if (data.enabled !== undefined) {
        if (typeof data.enabled !== 'boolean') {
          throw new Error('enabled must be a boolean')
        }
        updateData.enabled = data.enabled
      }

      return daemonClient.updateMemory(id, updateData)
    },
  )

  ipcMain.handle(IPC_CHANNELS.MEMORIES_DELETE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid memory ID')
    }
    return daemonClient.deleteMemory(id)
  })

  ipcMain.handle(IPC_CHANNELS.MEMORIES_ENABLE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid memory ID')
    }
    return daemonClient.enableMemory(id)
  })

  ipcMain.handle(IPC_CHANNELS.MEMORIES_DISABLE, async (_e, id: string) => {
    if (!isValidId(id)) {
      throw new Error('Invalid memory ID')
    }
    return daemonClient.disableMemory(id)
  })
}
