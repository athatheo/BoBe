import type {
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
  ModelsListResponse,
  ModelActionResponse,
} from '../../types'

// =============================================================================
// TOOLS
// =============================================================================

export async function listTools(baseUrl: string): Promise<ToolListResponse> {
  const response = await fetch(`${baseUrl}/tools`)
  if (!response.ok) {
    throw new Error(`Failed to list tools: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<ToolListResponse>
}

export async function enableTool(baseUrl: string, name: string): Promise<ToolUpdateResponse> {
  const response = await fetch(`${baseUrl}/tools/${encodeURIComponent(name)}/enable`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to enable tool: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<ToolUpdateResponse>
}

export async function disableTool(baseUrl: string, name: string): Promise<ToolUpdateResponse> {
  const response = await fetch(`${baseUrl}/tools/${encodeURIComponent(name)}/disable`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to disable tool: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<ToolUpdateResponse>
}

// =============================================================================
// MCP SERVERS
// =============================================================================

export async function listMCPServers(baseUrl: string): Promise<MCPServerListResponse> {
  const response = await fetch(`${baseUrl}/tools/mcp`)
  if (!response.ok) {
    throw new Error(`Failed to list MCP servers: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MCPServerListResponse>
}

export async function listMCPConfigs(baseUrl: string): Promise<MCPConfigListResponse> {
  const response = await fetch(`${baseUrl}/mcp-configs`)
  if (!response.ok) {
    throw new Error(`Failed to list MCP configs: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MCPConfigListResponse>
}

export async function createMCPServer(
  baseUrl: string,
  data: MCPServerCreateRequest,
): Promise<MCPServerCreateResponse> {
  const response = await fetch(`${baseUrl}/tools/mcp`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to create MCP server: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MCPServerCreateResponse>
}

export async function updateMCPServer(
  baseUrl: string,
  id: string,
  data: MCPServerUpdateRequest,
): Promise<MCPServerUpdateResponse> {
  const response = await fetch(`${baseUrl}/mcp-configs/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to update MCP server: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MCPServerUpdateResponse>
}

export async function deleteMCPServer(
  baseUrl: string,
  name: string,
): Promise<MCPServerDeleteResponse> {
  const response = await fetch(`${baseUrl}/tools/mcp/${encodeURIComponent(name)}`, {
    method: 'DELETE',
  })
  if (!response.ok) {
    throw new Error(`Failed to delete MCP server: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MCPServerDeleteResponse>
}

export async function reconnectMCPServer(
  baseUrl: string,
  name: string,
): Promise<MCPServerReconnectResponse> {
  const response = await fetch(`${baseUrl}/tools/mcp/${encodeURIComponent(name)}/reconnect`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to reconnect MCP server: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MCPServerReconnectResponse>
}

// =============================================================================
// MODELS
// =============================================================================

export async function getModels(baseUrl: string): Promise<ModelsListResponse> {
  const response = await fetch(`${baseUrl}/models`)
  if (!response.ok) {
    throw new Error(`Failed to get models: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<ModelsListResponse>
}

export async function getRegistryModels(baseUrl: string): Promise<ModelsListResponse> {
  const response = await fetch(`${baseUrl}/models/registry`)
  if (!response.ok) {
    throw new Error(`Failed to get registry models: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<ModelsListResponse>
}

export async function pullModel(baseUrl: string, modelName: string): Promise<ModelActionResponse> {
  const response = await fetch(`${baseUrl}/models/pull`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model: modelName }),
  })
  if (!response.ok) {
    throw new Error(`Failed to pull model: ${response.status} ${response.statusText}`)
  }

  const reader = response.body?.getReader()
  if (reader) {
    while (true) {
      const { done } = await reader.read()
      if (done) break
    }
  }

  return { ok: true, message: `Model ${modelName} pulled successfully` }
}

export async function deleteModel(
  baseUrl: string,
  modelName: string,
): Promise<ModelActionResponse> {
  const response = await fetch(`${baseUrl}/models/${encodeURIComponent(modelName)}`, {
    method: 'DELETE',
  })
  if (!response.ok) {
    throw new Error(`Failed to delete model: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<ModelActionResponse>
}
