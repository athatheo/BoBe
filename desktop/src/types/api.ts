/**
 * BoBe Daemon API Types
 * Base URL: http://localhost:8766
 *
 * Types for all daemon communication - matches Python backend API schema.
 */

// Re-export IndicatorType from bobe.ts to avoid duplication
import type { IndicatorType } from './bobe'
export type { IndicatorType }

// =============================================================================
// CORE ENUMS
// =============================================================================

/**
 * SSE event types from GET /events
 */
export type EventType =
  | 'indicator'
  | 'text_delta'
  | 'tool_call'
  | 'error'
  | 'heartbeat'
  | 'conversation_closed'
  | 'action_request'

// =============================================================================
// SSE EVENT STREAM (GET /events)
// =============================================================================

/** All SSE events wrapped in this envelope */
export interface StreamBundle {
  type: EventType
  payload: EventPayload
  message_id: string
  timestamp: string
}

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

export interface ErrorPayload {
  code: string
  message: string
  recoverable: boolean
  details: Record<string, unknown> | null
}

export interface HeartbeatPayload {}

export interface ConversationClosedPayload {
  conversation_id: string
  reason: string // e.g., "inactivity_timeout"
  turn_count: number
}

export interface ActionRequestPayload {
  action: string
  prompt: string
  request_id: string
  timeout_ms: number
  options?: string[]
}

// =============================================================================
export type EventPayload =
  | IndicatorPayload
  | TextDeltaPayload
  | ToolCallPayload
  | ErrorPayload
  | HeartbeatPayload
  | ConversationClosedPayload
  | ActionRequestPayload

// =============================================================================
// GOALS
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

/** GET /goals */
export interface GoalListResponse {
  goals: Goal[]
  count: number
  active_count: number
}

/** POST /goals */
export interface GoalCreateRequest {
  content: string
  priority?: GoalPriority
  enabled?: boolean
}

/** PATCH /goals/{id} */
export interface GoalUpdateRequest {
  content?: string
  status?: GoalStatus
  priority?: GoalPriority
  enabled?: boolean
}

/** POST /goals/{id}/complete, /archive, DELETE /goals/{id} */
export interface GoalActionResponse {
  id: string
  status: string
  message: string
}

// =============================================================================
// GOAL PLANS
// =============================================================================

export type GoalPlanStatus = 'pending_approval' | 'approved' | 'auto_approved' | 'in_progress' | 'completed' | 'failed' | 'rejected'
export type GoalPlanStepStatus = 'pending' | 'in_progress' | 'completed' | 'failed' | 'skipped'

export interface GoalPlan {
  id: string
  goal_id: string
  summary: string
  status: GoalPlanStatus
  failure_count: number
  last_error: string | null
  created_at: string
  updated_at: string
  steps?: GoalPlanStep[]
}

export interface GoalPlanStep {
  id: string
  plan_id: string
  step_order: number
  content: string
  status: GoalPlanStepStatus
  result: string | null
  error: string | null
  started_at: string | null
  completed_at: string | null
}

// =============================================================================
// SOULS (BoBe's Personality)
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

/** GET /souls */
export interface SoulListResponse {
  souls: Soul[]
  count: number
  enabled_count: number
}

/** POST /souls */
export interface SoulCreateRequest {
  name: string
  content: string
  enabled?: boolean
}

/** PATCH /souls/{id} */
export interface SoulUpdateRequest {
  content?: string
  enabled?: boolean
}

/** POST /souls/{id}/enable, /disable, DELETE /souls/{id} */
export interface SoulActionResponse {
  id: string
  name: string
  enabled: boolean
  message: string
}

// =============================================================================
// USER PROFILES
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

/** GET /user-profiles */
export interface UserProfileListResponse {
  profiles: UserProfile[]
  count: number
  enabled_count: number
}

/** POST /user-profiles */
export interface UserProfileCreateRequest {
  name: string
  content: string
  enabled?: boolean
}

/** PATCH /user-profiles/{id} */
export interface UserProfileUpdateRequest {
  content?: string
  enabled?: boolean
}

/** POST /user-profiles/{id}/enable, /disable, DELETE */
export interface UserProfileActionResponse {
  id: string
  name: string
  enabled: boolean
  message: string
}

// =============================================================================
// TOOLS
// =============================================================================

export interface Tool {
  name: string
  description: string
  provider: string
  enabled: boolean
  category?: string
}

/** GET /tools */
export interface ToolListResponse {
  tools: Tool[]
  count: number
  providers: string[]
}

/** PATCH /tools/{name}, POST /tools/{name}/enable, POST /tools/{name}/disable */
export interface ToolUpdateResponse {
  name: string
  enabled: boolean
  message: string
}

// =============================================================================
// MCP SERVERS
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

/** GET /tools/mcp */
export interface MCPServerListResponse {
  servers: MCPServer[]
  count: number
  connected_count: number
}

/** GET /mcp-configs */
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

/** POST /tools/mcp */
export interface MCPServerCreateRequest {
  name: string
  command: string
  args?: string[]
  env?: Record<string, string>
  enabled?: boolean
  excluded_tools?: string[]
}

/** PATCH /tools/mcp/{name} */
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

/** DELETE /tools/mcp/{name} */
export interface MCPServerDeleteResponse {
  name: string
  message: string
}

/** POST /tools/mcp/{name}/reconnect */
export interface MCPServerReconnectResponse {
  name: string
  connected: boolean
  tool_count: number
  message: string
  error?: string
}

// =============================================================================
// SETTINGS (Daemon Configuration)
// =============================================================================

/** GET /settings — all user-facing settings from live config */
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
  // Goal Worker
  goal_worker_enabled: boolean
  goal_worker_autonomous: boolean
  goal_worker_max_concurrent: number
  projects_dir: string
}

/** PATCH /settings — all fields optional, only provided fields are updated */
export interface SettingsUpdateRequest {
  // LLM backend + model (triggers provider rebuild — instant, no restart)
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
  // Goal Worker
  goal_worker_enabled?: boolean
  goal_worker_autonomous?: boolean
  goal_worker_max_concurrent?: number
  projects_dir?: string
}

export interface SettingsUpdateResponse {
  message: string
  applied_fields: string[]
  restart_required_fields: string[]
}

// =============================================================================
// MODELS (LLM Model Management)
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
// MEMORIES
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

/** GET /memories */
export interface MemoryListResponse {
  memories: Memory[]
  count: number
  total: number
}

/** GET /memories query parameters */
export interface MemoryListParams {
  memory_type?: MemoryType
  category?: MemoryCategory
  source?: MemorySource
  enabled_only?: boolean
  limit?: number
  offset?: number
}

/** POST /memories */
export interface MemoryCreateRequest {
  content: string
  category?: MemoryCategory
  memory_type?: MemoryType
}

/** PATCH /memories/{id} */
export interface MemoryUpdateRequest {
  content?: string
  category?: MemoryCategory
  enabled?: boolean
}

/** POST /memories/{id}/enable, /disable, DELETE /memories/{id} */
export interface MemoryActionResponse {
  id: string
  enabled: boolean
  message: string
}
