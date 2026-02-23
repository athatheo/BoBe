/**
 * Daemon API types - matches Python backend schema exactly
 *
 * These types define the contract between the Electron shell and the Python daemon.
 * See: bobe-dev schema for the authoritative source.
 */

// =============================================================================
// SSE EVENT TYPES
// =============================================================================

export type EventType =
  | 'indicator'
  | 'text_delta'
  | 'error'
  | 'heartbeat'
  | 'tool_call'
  | 'conversation_closed'

/**
 * Indicator types matching Python daemon.
 * IMPORTANT: Keep in sync with src/types/bobe.ts IndicatorType
 */
export type IndicatorType =
  | 'idle'
  | 'capturing'
  | 'analyzing'
  | 'thinking'
  | 'generating'
  | 'speaking'

// =============================================================================
// SSE PAYLOADS
// =============================================================================

export interface IndicatorPayload {
  indicator: IndicatorType
  message: string | null
  progress: number | null
}

export interface TextDeltaPayload {
  delta: string
  sequence: number
  done: boolean
  // Note: message_id is at StreamBundle level, not in payload
}

export interface ErrorPayload {
  code: string
  message: string
  recoverable: boolean
  details: Record<string, unknown> | null
}

export interface HeartbeatPayload {
  // Empty payload
}

export interface ToolCallStartPayload {
  status: 'start'
  tool_name: string
  tool_call_id: string
}

export interface ToolCallCompletePayload {
  status: 'complete'
  tool_name: string
  tool_call_id: string
  success: boolean
  error: string | null
  duration_ms: number | null
}

export type ToolCallPayload = ToolCallStartPayload | ToolCallCompletePayload

export interface ConversationClosedPayload {
  conversation_id: string
  reason: string // e.g., "inactivity_timeout"
  turn_count: number
}

// =============================================================================
// STREAM BUNDLE (SSE EVENT ENVELOPE)
// =============================================================================

export interface StreamBundleBase {
  message_id: string
  timestamp: string
}

export interface IndicatorBundle extends StreamBundleBase {
  type: 'indicator'
  payload: IndicatorPayload
}

export interface TextDeltaBundle extends StreamBundleBase {
  type: 'text_delta'
  payload: TextDeltaPayload
}

export interface ErrorBundle extends StreamBundleBase {
  type: 'error'
  payload: ErrorPayload
}

export interface HeartbeatBundle extends StreamBundleBase {
  type: 'heartbeat'
  payload: HeartbeatPayload
}

export interface ToolCallBundle extends StreamBundleBase {
  type: 'tool_call'
  payload: ToolCallPayload
}

export interface ConversationClosedBundle extends StreamBundleBase {
  type: 'conversation_closed'
  payload: ConversationClosedPayload
}

export type StreamBundle =
  | IndicatorBundle
  | TextDeltaBundle
  | ErrorBundle
  | HeartbeatBundle
  | ToolCallBundle
  | ConversationClosedBundle

// =============================================================================
// HTTP RESPONSE TYPES
// =============================================================================

export interface HealthResponse {
  status: string
  version: string
  llm_available: boolean
  database_ok: boolean
  capture_ok: boolean
}

export interface StatusResponse {
  indicator: IndicatorType
  capturing: boolean
  version: string
}

export interface CaptureResponse {
  capturing: boolean
  message: string
}

export interface MessageResponse {
  message_id: string
}

export interface ContextItemResponse {
  id: string
  source: string
  content: string
  summary?: string | null
  category: string
  importance: number
  created_at: string
}

// =============================================================================
// GOALS API
// =============================================================================

export type GoalStatus = 'active' | 'completed' | 'archived'
export type GoalPriority = 'high' | 'medium' | 'low'
export type GoalSource = 'user' | 'inferred'

export interface Goal {
  id: string
  content: string
  status: GoalStatus
  priority: GoalPriority
  source: GoalSource
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface GoalListResponse {
  goals: Goal[]
  count: number
  active_count: number
}

export interface GoalCreateRequest {
  content: string
  priority?: GoalPriority
  enabled?: boolean
}

export interface GoalUpdateRequest {
  content?: string
  status?: GoalStatus
  priority?: GoalPriority
  enabled?: boolean
}

export interface GoalActionResponse {
  id: string
  status: string
  message: string
}

// =============================================================================
// SOULS API
// =============================================================================

export interface Soul {
  id: string
  name: string
  content: string
  enabled: boolean
  is_default: boolean
  created_at: string
  updated_at: string
}

export interface SoulListResponse {
  souls: Soul[]
  count: number
  enabled_count: number
}

export interface SoulCreateRequest {
  name: string
  content: string
  enabled?: boolean
}

export interface SoulUpdateRequest {
  content?: string
  enabled?: boolean
}

export interface SoulActionResponse {
  id: string
  name: string
  enabled: boolean
  message: string
}

// =============================================================================
// USER PROFILES API
// =============================================================================

export interface UserProfile {
  id: string
  name: string
  content: string
  enabled: boolean
  is_default: boolean
  created_at: string
  updated_at: string
}

export interface UserProfileListResponse {
  profiles: UserProfile[]
  count: number
  enabled_count: number
}

export interface UserProfileCreateRequest {
  name: string
  content: string
  enabled?: boolean
}

export interface UserProfileUpdateRequest {
  content?: string
  enabled?: boolean
}

export interface UserProfileActionResponse {
  id: string
  name: string
  enabled: boolean
  message: string
}

// =============================================================================
// TOOLS API
// =============================================================================

export interface Tool {
  name: string
  description: string
  provider: string
  enabled: boolean
  category?: string
}

export interface ToolListResponse {
  tools: Tool[]
  count: number
  providers: string[]
}

export interface ToolUpdateResponse {
  name: string
  enabled: boolean
  message: string
}

// =============================================================================
// MCP SERVERS API
// =============================================================================

export interface MCPServer {
  id: string
  name: string
  command: string
  args: string[]
  connected: boolean
  enabled: boolean
  tool_count: number
  excluded_tools: string[]
  error?: string
}

export interface MCPServerListResponse {
  servers: MCPServer[]
  count: number
  connected_count: number
}

export interface MCPConfig {
  id: string
  server_name: string
  excluded_tools: string[]
}

export interface MCPConfigListResponse {
  configs: MCPConfig[]
  count: number
  enabled_count: number
}

export interface MCPServerCreateRequest {
  name: string
  command: string
  args?: string[]
  env?: Record<string, string>
  enabled?: boolean
  excluded_tools?: string[]
}

export interface MCPServerUpdateRequest {
  excluded_tools?: string[]
}

export interface MCPServerUpdateResponse {
  name: string
  excluded_tools: string[]
  message: string
}

export interface MCPServerCreateResponse {
  name: string
  connected: boolean
  tool_count: number
  message: string
  error?: string
}

export interface MCPServerDeleteResponse {
  name: string
  message: string
}

export interface MCPServerReconnectResponse {
  name: string
  connected: boolean
  tool_count: number
  message: string
  error?: string
}

// =============================================================================
// SETTINGS API
// =============================================================================

export interface DaemonSettings {
  // LLM
  llm_backend: string
  ollama_model: string
  openai_model: string
  openai_api_key_set: boolean
  azure_openai_endpoint: string
  azure_openai_deployment: string
  azure_openai_api_key_set: boolean
  // Capture
  capture_enabled: boolean
  capture_interval_seconds: number
  // Check-in
  checkin_enabled: boolean
  checkin_times: string[]
  checkin_jitter_minutes: number
  // Learning
  learning_enabled: boolean
  learning_interval_minutes: number
  // Conversation
  conversation_inactivity_timeout_seconds: number
  conversation_auto_close_minutes: number
  conversation_summary_enabled: boolean
  // Goals
  goal_check_interval_seconds: number
  // Projects
  projects_directory: string
  // Tools
  tools_enabled: boolean
  tools_max_iterations: number
  // MCP
  mcp_enabled: boolean
  // Similarity thresholds
  similarity_deduplication_threshold: number
  similarity_search_recall_threshold: number
  similarity_clustering_threshold: number
  // Memory retention
  memory_short_term_retention_days: number
  memory_long_term_retention_days: number
}

export interface SettingsUpdateRequest {
  // LLM backend + model
  llm_backend?: 'local' | 'openai' | 'ollama' | 'azure_openai'
  ollama_model?: string
  openai_model?: string
  openai_api_key?: string
  azure_openai_endpoint?: string
  azure_openai_deployment?: string
  azure_openai_api_key?: string
  // Capture
  capture_enabled?: boolean
  capture_interval_seconds?: number
  // Check-in
  checkin_enabled?: boolean
  checkin_times?: string[]
  checkin_jitter_minutes?: number
  // Learning
  learning_enabled?: boolean
  learning_interval_minutes?: number
  // Conversation
  conversation_inactivity_timeout_seconds?: number
  conversation_auto_close_minutes?: number
  conversation_summary_enabled?: boolean
  // Goals
  goal_check_interval_seconds?: number
  // Projects
  projects_directory?: string
  // Tools
  tools_enabled?: boolean
  tools_max_iterations?: number
  // MCP
  mcp_enabled?: boolean
  // Similarity thresholds
  similarity_deduplication_threshold?: number
  similarity_search_recall_threshold?: number
  similarity_clustering_threshold?: number
  // Memory retention
  memory_short_term_retention_days?: number
  memory_long_term_retention_days?: number
}

export interface SettingsUpdateResponse {
  message: string
  applied_fields: string[]
  restart_required_fields: string[]
}

// =============================================================================
// MODELS API
// =============================================================================

export interface ModelInfo {
  name: string
  size_bytes: number
  modified_at: string
}

export interface ModelsListResponse {
  backend: string
  models: ModelInfo[]
  supports_pull: boolean
}

export interface ModelActionResponse {
  ok: boolean
  message: string
}

// =============================================================================
// MEMORIES API
// =============================================================================

export type MemoryType = 'short_term' | 'long_term' | 'explicit'
export type MemoryCategory =
  | 'preference'
  | 'pattern'
  | 'fact'
  | 'interest'
  | 'general'
  | 'observation'
export type MemorySource = 'observation' | 'conversation' | 'user' | 'visual_diary'

export interface Memory {
  id: string
  content: string
  memory_type: MemoryType
  category: MemoryCategory
  source: MemorySource
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface MemoryListResponse {
  memories: Memory[]
  count: number
  total: number
}

export interface MemoryListParams {
  memory_type?: MemoryType
  category?: MemoryCategory
  source?: MemorySource
  enabled_only?: boolean
  limit?: number
  offset?: number
}

export interface MemoryCreateRequest {
  content: string
  category?: MemoryCategory
  memory_type?: MemoryType
}

export interface MemoryUpdateRequest {
  content?: string
  category?: MemoryCategory
  enabled?: boolean
}

export interface MemoryActionResponse {
  id: string
  enabled: boolean
  message: string
}

// =============================================================================
// CLIENT EVENT TYPES (emitted by DaemonClient)
// =============================================================================

export type DaemonClientEvent =
  | 'connected'
  | 'disconnected'
  | 'reconnecting'
  | 'indicator'
  | 'text_delta'
  | 'message_complete'
  | 'error'
  | 'heartbeat'
  | 'tool_call'
  | 'conversation_closed'

export interface DaemonClientEventMap {
  connected: void
  disconnected: void
  reconnecting: { attempt: number; maxAttempts: number }
  indicator: IndicatorPayload
  text_delta: TextDeltaPayload & { message_id: string } // message_id added from bundle level
  message_complete: { message_id: string; content: string }
  error: ErrorPayload
  heartbeat: void
  tool_call: ToolCallPayload
  conversation_closed: ConversationClosedPayload
}
