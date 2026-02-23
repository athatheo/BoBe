/**
 * Window type augmentation for contextBridge API
 */

import type {
  BobeAPI,
  SetupAPI,
  GoalsAPI,
  SoulsAPI,
  UserProfilesAPI,
  ToolsAPI,
  MCPServersAPI,
  SettingsAPI,
  MemoriesAPI,
  AppDataAPI,
  PermissionsAPI,
} from './ipc'

declare global {
  interface Window {
    bobe: BobeAPI
    setup: SetupAPI
    appData: AppDataAPI
    goals: GoalsAPI
    souls: SoulsAPI
    userProfiles: UserProfilesAPI
    tools: ToolsAPI
    mcpServers: MCPServersAPI
    settings: SettingsAPI
    memories: MemoriesAPI
    permissions: PermissionsAPI
  }
}

export {}
