import type { DaemonSettings, SettingsUpdateRequest, SettingsUpdateResponse } from '../../types'

export async function getSettings(baseUrl: string): Promise<DaemonSettings> {
  const response = await fetch(`${baseUrl}/settings`)
  if (!response.ok) {
    throw new Error(`Failed to get settings: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<DaemonSettings>
}

export async function updateSettings(
  baseUrl: string,
  data: SettingsUpdateRequest,
): Promise<SettingsUpdateResponse> {
  const response = await fetch(`${baseUrl}/settings`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!response.ok) {
    throw new Error(`Failed to update settings: ${response.status} ${response.statusText}`)
  }
  return response.json() as Promise<SettingsUpdateResponse>
}
