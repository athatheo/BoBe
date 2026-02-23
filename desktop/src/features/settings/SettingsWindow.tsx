/**
 * SettingsWindow component
 *
 * Main layout for the Bobe Tuning settings panel.
 * Defaults to overview landing page. Monaco-using components
 * are lazy-loaded so the window opens instantly.
 */

import { Component, useState, Suspense, lazy } from 'react'
import type { ReactNode } from 'react'
import { RefreshCw } from 'lucide-react'
import { SettingsSidebar } from './SettingsSidebar'
import { SettingsOverview } from './SettingsOverview'
import { AppearanceSettings } from './AppearanceSettings'
import { AIModelSettings } from './AIModelSettings'
import { BehaviorSettings } from './BehaviorSettings'
import { AdvancedSettings } from './AdvancedSettings'
import { PreferencesPlaceholder } from './PreferencesPlaceholder'
import { PrivacySettings } from './PrivacySettings'
import type { SettingsCategory } from '@/types/settings'

// Lazy-load Monaco-heavy components (Souls, Goals, Memories, UserProfiles, Tools, MCP)
const SoulsSettings = lazy(() =>
  import('./SoulsSettings').then((m) => ({ default: m.SoulsSettings })),
)
const GoalsSettings = lazy(() =>
  import('./GoalsSettings').then((m) => ({ default: m.GoalsSettings })),
)
const MemoriesSettings = lazy(() =>
  import('./MemoriesSettings').then((m) => ({ default: m.MemoriesSettings })),
)
const UserProfilesSettings = lazy(() =>
  import('./UserProfilesSettings').then((m) => ({ default: m.UserProfilesSettings })),
)
const ToolsSettings = lazy(() =>
  import('./ToolsSettings').then((m) => ({ default: m.ToolsSettings })),
)
const MCPServersSettings = lazy(() =>
  import('./MCPServersSettings').then((m) => ({ default: m.MCPServersSettings })),
)

// ErrorBoundary to catch lazy-load failures that Suspense can't handle
class LazyErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null }
  static getDerivedStateFromError(error: Error) {
    return { error }
  }
  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 24, color: 'var(--color-bobe-terracotta)' }}>
          <strong>Failed to load component</strong>
          <pre style={{ marginTop: 8, fontSize: 12, whiteSpace: 'pre-wrap' }}>
            {this.state.error.message}
          </pre>
        </div>
      )
    }
    return this.props.children
  }
}

// Active category — null means overview/landing page
type ActiveView = SettingsCategory | null

// Categories that use the document list + editor pattern
const CONTENT_CATEGORIES = [
  'souls',
  'goals',
  'memories',
  'user-profiles',
  'tools',
  'mcp-servers',
] as const
const PREFERENCE_CATEGORIES = ['appearance', 'ai-model', 'behavior', 'privacy'] as const
const ADVANCED_CATEGORIES = ['advanced'] as const

function isContentCategory(cat: SettingsCategory): boolean {
  return CONTENT_CATEGORIES.includes(cat as (typeof CONTENT_CATEGORIES)[number])
}

function isPreferenceCategory(cat: SettingsCategory): boolean {
  return PREFERENCE_CATEGORIES.includes(cat as (typeof PREFERENCE_CATEGORIES)[number])
}

function isAdvancedCategory(cat: SettingsCategory): boolean {
  return ADVANCED_CATEGORIES.includes(cat as (typeof ADVANCED_CATEGORIES)[number])
}

// Title mapping
const CATEGORY_TITLES: Record<SettingsCategory, string> = {
  souls: 'Souls',
  goals: 'Goals',
  memories: 'Memories',
  'user-profiles': 'User Profiles',
  tools: 'Tools',
  'mcp-servers': 'MCP Servers',
  appearance: 'Appearance',
  'ai-model': 'AI Model',
  behavior: 'Behavior',
  privacy: 'Privacy',
  advanced: 'For Nerds',
}

function LazyFallback() {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        width: '100%',
        height: '100%',
        gap: 8,
        color: 'var(--color-bobe-clay)',
      }}
    >
      <RefreshCw size={18} className="animate-spin" />
      <span style={{ fontSize: 13 }}>Loading...</span>
    </div>
  )
}

export function SettingsWindow() {
  const [activeView, setActiveView] = useState<ActiveView>(null)

  const handleCategoryChange = (category: SettingsCategory) => {
    setActiveView(category)
  }

  const isOverview = activeView === null
  const isContent = activeView !== null && isContentCategory(activeView)
  const isPreference = activeView !== null && isPreferenceCategory(activeView)
  const isAdvanced = activeView !== null && isAdvancedCategory(activeView)

  return (
    <div className="settings-layout">
      <SettingsSidebar activeCategory={activeView} onCategoryChange={handleCategoryChange} />

      <main className="settings-content">
        {/* Header with title */}
        <div className="settings-header drag-region">
          <div className="settings-header-title">
            <h2 className="settings-page-title">{isOverview ? '' : CATEGORY_TITLES[activeView]}</h2>
          </div>
        </div>

        {/* Content area */}
        {isOverview ? (
          <div className="settings-preferences-content">
            <SettingsOverview onNavigate={handleCategoryChange} />
          </div>
        ) : isAdvanced ? (
          <div className="settings-preferences-content">
            <AdvancedSettings />
          </div>
        ) : isPreference ? (
          <div className="settings-preferences-content">
            {activeView === 'appearance' ? (
              <AppearanceSettings />
            ) : activeView === 'ai-model' ? (
              <AIModelSettings />
            ) : activeView === 'behavior' ? (
              <BehaviorSettings />
            ) : activeView === 'privacy' ? (
              <PrivacySettings />
            ) : (
              <PreferencesPlaceholder category={activeView} />
            )}
          </div>
        ) : isContent ? (
          <div className="settings-main">
            <LazyErrorBoundary>
              <Suspense fallback={<LazyFallback />}>
                {activeView === 'souls' && <SoulsSettings />}
                {activeView === 'goals' && <GoalsSettings />}
                {activeView === 'memories' && <MemoriesSettings />}
                {activeView === 'user-profiles' && <UserProfilesSettings />}
                {activeView === 'tools' && <ToolsSettings />}
                {activeView === 'mcp-servers' && <MCPServersSettings />}
              </Suspense>
            </LazyErrorBoundary>
          </div>
        ) : null}
      </main>
    </div>
  )
}
