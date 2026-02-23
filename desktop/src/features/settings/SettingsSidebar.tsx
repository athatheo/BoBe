/**
 * SettingsSidebar component
 *
 * Navigation sidebar for settings categories.
 * Groups categories into Context, Integrations, and Preferences sections.
 */

import {
  Sparkles,
  Target,
  Brain,
  User,
  Wrench,
  Server,
  Palette,
  Cpu,
  SlidersHorizontal,
  Mic,
  Shield,
  Terminal,
} from 'lucide-react'
import { cn } from '@/lib/cn'
import type { SettingsCategory } from '@/types/settings'
import { SETTINGS_CATEGORIES } from '@/types/settings'

interface SettingsSidebarProps {
  activeCategory: SettingsCategory | null
  onCategoryChange: (category: SettingsCategory) => void
}

// Icon mapping
const ICONS: Record<string, React.ReactNode> = {
  sparkles: <Sparkles size={18} />,
  target: <Target size={18} />,
  brain: <Brain size={18} />,
  user: <User size={18} />,
  wrench: <Wrench size={18} />,
  server: <Server size={18} />,
  palette: <Palette size={18} />,
  cpu: <Cpu size={18} />,
  sliders: <SlidersHorizontal size={18} />,
  mic: <Mic size={18} />,
  shield: <Shield size={18} />,
  terminal: <Terminal size={18} />,
}

export function SettingsSidebar({ activeCategory, onCategoryChange }: SettingsSidebarProps) {
  const contextCategories = SETTINGS_CATEGORIES.filter((c) => c.group === 'context')
  const integrationCategories = SETTINGS_CATEGORIES.filter((c) => c.group === 'integrations')
  const preferenceCategories = SETTINGS_CATEGORIES.filter((c) => c.group === 'preferences')
  const advancedCategories = SETTINGS_CATEGORIES.filter((c) => c.group === 'advanced')

  return (
    <aside className="settings-sidebar">
      {/* Title area - accounts for macOS traffic lights */}
      <div className="settings-sidebar-header drag-region">
        <h1 className="settings-sidebar-title">BOBE TUNING</h1>
      </div>

      <nav className="settings-nav">
        {/* Context section */}
        <div className="settings-nav-section">
          <span className="settings-nav-section-label">CONTEXT</span>
          <ul className="settings-nav-list">
            {contextCategories.map((category) => (
              <NavItem
                key={category.id}
                category={category}
                isActive={activeCategory === category.id}
                onClick={() => onCategoryChange(category.id)}
              />
            ))}
          </ul>
        </div>

        {/* Integrations section */}
        <div className="settings-nav-section">
          <span className="settings-nav-section-label">INTEGRATIONS</span>
          <ul className="settings-nav-list">
            {integrationCategories.map((category) => (
              <NavItem
                key={category.id}
                category={category}
                isActive={activeCategory === category.id}
                onClick={() => onCategoryChange(category.id)}
              />
            ))}
          </ul>
        </div>

        {/* Preferences section */}
        <div className="settings-nav-section">
          <span className="settings-nav-section-label">PREFERENCES</span>
          <ul className="settings-nav-list">
            {preferenceCategories.map((category) => (
              <NavItem
                key={category.id}
                category={category}
                isActive={activeCategory === category.id}
                onClick={() => onCategoryChange(category.id)}
              />
            ))}
          </ul>
        </div>

        {/* Advanced section */}
        <div className="settings-nav-section">
          <span className="settings-nav-section-label">ADVANCED</span>
          <ul className="settings-nav-list">
            {advancedCategories.map((category) => (
              <NavItem
                key={category.id}
                category={category}
                isActive={activeCategory === category.id}
                onClick={() => onCategoryChange(category.id)}
              />
            ))}
          </ul>
        </div>
      </nav>
    </aside>
  )
}

// =============================================================================
// NAV ITEM
// =============================================================================

interface NavItemProps {
  category: (typeof SETTINGS_CATEGORIES)[number]
  isActive: boolean
  onClick: () => void
}

function NavItem({ category, isActive, onClick }: NavItemProps) {
  return (
    <li>
      <button
        onClick={onClick}
        className={cn('settings-nav-item', isActive && 'settings-nav-item-active')}
      >
        <span className={cn('settings-nav-icon', isActive && 'settings-nav-icon-active')}>
          {ICONS[category.icon]}
        </span>
        <span className="settings-nav-label">{category.label}</span>
      </button>
    </li>
  )
}
