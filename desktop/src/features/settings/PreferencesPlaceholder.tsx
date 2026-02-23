/**
 * PreferencesPlaceholder component
 *
 * Placeholder for preferences that aren't implemented yet.
 */

import { Construction } from 'lucide-react'
import type { SettingsCategory } from '@/types/settings'

interface PreferencesPlaceholderProps {
  category: SettingsCategory
}

const CATEGORY_INFO: Record<string, { title: string; description: string }> = {
  behavior: {
    title: 'Behavior Settings',
    description: 'Configure startup behavior, hotkeys, and notification preferences.',
  },
  privacy: {
    title: 'Privacy Settings',
    description: 'Control data handling, local storage, and telemetry options.',
  },
}

export function PreferencesPlaceholder({ category }: PreferencesPlaceholderProps) {
  const info = CATEGORY_INFO[category] || {
    title: 'Settings',
    description: 'Configure your preferences.',
  }

  return (
    <div className="preferences-panel">
      <div className="preferences-placeholder">
        <Construction size={48} className="preferences-placeholder-icon" />
        <h3 className="preferences-placeholder-title">{info.title}</h3>
        <p className="preferences-placeholder-description">{info.description}</p>
        <span className="preferences-placeholder-badge">Coming Soon</span>
      </div>
    </div>
  )
}
