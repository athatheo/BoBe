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
  Memory,
  MemoryListResponse,
  MemoryListParams,
  MemoryCreateRequest,
  MemoryUpdateRequest,
  MemoryActionResponse,
} from '../../types'

// =============================================================================
// GOALS
// =============================================================================

export async function listGoals(baseUrl: string): Promise<GoalListResponse> {
  const response = await fetch(`${baseUrl}/goals`)
  if (!response.ok) {
    throw new Error(`Failed to list goals: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<GoalListResponse>
}

export async function getGoal(baseUrl: string, id: string): Promise<Goal> {
  const response = await fetch(`${baseUrl}/goals/${encodeURIComponent(id)}`)
  if (!response.ok) {
    throw new Error(`Failed to get goal: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Goal>
}

export async function createGoal(baseUrl: string, data: GoalCreateRequest): Promise<Goal> {
  const response = await fetch(`${baseUrl}/goals`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to create goal: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Goal>
}

export async function updateGoal(
  baseUrl: string,
  id: string,
  data: GoalUpdateRequest,
): Promise<Goal> {
  const response = await fetch(`${baseUrl}/goals/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to update goal: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Goal>
}

export async function deleteGoal(baseUrl: string, id: string): Promise<GoalActionResponse> {
  const response = await fetch(`${baseUrl}/goals/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
  if (!response.ok) {
    throw new Error(`Failed to delete goal: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<GoalActionResponse>
}

export async function completeGoal(baseUrl: string, id: string): Promise<GoalActionResponse> {
  const response = await fetch(`${baseUrl}/goals/${encodeURIComponent(id)}/complete`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to complete goal: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<GoalActionResponse>
}

export async function archiveGoal(baseUrl: string, id: string): Promise<GoalActionResponse> {
  const response = await fetch(`${baseUrl}/goals/${encodeURIComponent(id)}/archive`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to archive goal: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<GoalActionResponse>
}

// =============================================================================
// SOULS
// =============================================================================

export async function listSouls(baseUrl: string): Promise<SoulListResponse> {
  const response = await fetch(`${baseUrl}/souls`)
  if (!response.ok) {
    throw new Error(`Failed to list souls: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<SoulListResponse>
}

export async function getSoul(baseUrl: string, id: string): Promise<Soul> {
  const response = await fetch(`${baseUrl}/souls/${encodeURIComponent(id)}`)
  if (!response.ok) {
    throw new Error(`Failed to get soul: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Soul>
}

export async function createSoul(baseUrl: string, data: SoulCreateRequest): Promise<Soul> {
  const response = await fetch(`${baseUrl}/souls`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to create soul: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Soul>
}

export async function updateSoul(
  baseUrl: string,
  id: string,
  data: SoulUpdateRequest,
): Promise<Soul> {
  const response = await fetch(`${baseUrl}/souls/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to update soul: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Soul>
}

export async function deleteSoul(baseUrl: string, id: string): Promise<SoulActionResponse> {
  const response = await fetch(`${baseUrl}/souls/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
  if (!response.ok) {
    throw new Error(`Failed to delete soul: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<SoulActionResponse>
}

export async function enableSoul(baseUrl: string, id: string): Promise<SoulActionResponse> {
  const response = await fetch(`${baseUrl}/souls/${encodeURIComponent(id)}/enable`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to enable soul: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<SoulActionResponse>
}

export async function disableSoul(baseUrl: string, id: string): Promise<SoulActionResponse> {
  const response = await fetch(`${baseUrl}/souls/${encodeURIComponent(id)}/disable`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to disable soul: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<SoulActionResponse>
}

// =============================================================================
// USER PROFILES
// =============================================================================

export async function listUserProfiles(baseUrl: string): Promise<UserProfileListResponse> {
  const response = await fetch(`${baseUrl}/user-profiles`)
  if (!response.ok) {
    throw new Error(`Failed to list user profiles: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<UserProfileListResponse>
}

export async function getUserProfile(baseUrl: string, id: string): Promise<UserProfile> {
  const response = await fetch(`${baseUrl}/user-profiles/${encodeURIComponent(id)}`)
  if (!response.ok) {
    throw new Error(`Failed to get user profile: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<UserProfile>
}

export async function createUserProfile(
  baseUrl: string,
  data: UserProfileCreateRequest,
): Promise<UserProfile> {
  const response = await fetch(`${baseUrl}/user-profiles`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to create user profile: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<UserProfile>
}

export async function updateUserProfile(
  baseUrl: string,
  id: string,
  data: UserProfileUpdateRequest,
): Promise<UserProfile> {
  const response = await fetch(`${baseUrl}/user-profiles/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to update user profile: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<UserProfile>
}

export async function deleteUserProfile(
  baseUrl: string,
  id: string,
): Promise<UserProfileActionResponse> {
  const response = await fetch(`${baseUrl}/user-profiles/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
  if (!response.ok) {
    throw new Error(`Failed to delete user profile: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<UserProfileActionResponse>
}

export async function enableUserProfile(
  baseUrl: string,
  id: string,
): Promise<UserProfileActionResponse> {
  const response = await fetch(`${baseUrl}/user-profiles/${encodeURIComponent(id)}/enable`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to enable user profile: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<UserProfileActionResponse>
}

export async function disableUserProfile(
  baseUrl: string,
  id: string,
): Promise<UserProfileActionResponse> {
  const response = await fetch(`${baseUrl}/user-profiles/${encodeURIComponent(id)}/disable`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to disable user profile: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<UserProfileActionResponse>
}

// =============================================================================
// MEMORIES
// =============================================================================

export async function listMemories(
  baseUrl: string,
  params?: MemoryListParams,
): Promise<MemoryListResponse> {
  const url = new URL(`${baseUrl}/memories`)
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
  if (!response.ok) {
    throw new Error(`Failed to list memories: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MemoryListResponse>
}

export async function getMemory(baseUrl: string, id: string): Promise<Memory> {
  const response = await fetch(`${baseUrl}/memories/${encodeURIComponent(id)}`)
  if (!response.ok) {
    throw new Error(`Failed to get memory: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Memory>
}

export async function createMemory(baseUrl: string, data: MemoryCreateRequest): Promise<Memory> {
  const response = await fetch(`${baseUrl}/memories`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to create memory: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Memory>
}

export async function updateMemory(
  baseUrl: string,
  id: string,
  data: MemoryUpdateRequest,
): Promise<Memory> {
  const response = await fetch(`${baseUrl}/memories/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to update memory: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<Memory>
}

export async function deleteMemory(baseUrl: string, id: string): Promise<MemoryActionResponse> {
  const response = await fetch(`${baseUrl}/memories/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
  if (!response.ok) {
    throw new Error(`Failed to delete memory: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MemoryActionResponse>
}

export async function enableMemory(baseUrl: string, id: string): Promise<MemoryActionResponse> {
  const response = await fetch(`${baseUrl}/memories/${encodeURIComponent(id)}/enable`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to enable memory: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MemoryActionResponse>
}

export async function disableMemory(baseUrl: string, id: string): Promise<MemoryActionResponse> {
  const response = await fetch(`${baseUrl}/memories/${encodeURIComponent(id)}/disable`, {
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(`Failed to disable memory: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<MemoryActionResponse>
}
