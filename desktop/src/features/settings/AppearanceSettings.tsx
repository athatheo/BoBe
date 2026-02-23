/**
 * AppearanceSettings component
 *
 * Theme selection with visual previews.
 * Uses form-based layout (not document editor pattern).
 */

import { motion } from 'framer-motion'
import { Check, Palette } from 'lucide-react'
import { cn } from '@/lib/cn'
import { useTheme } from '@/hooks'
import type { ThemeConfig } from '@/types/settings'

export function AppearanceSettings() {
  const { themeId, themes, setTheme } = useTheme()

  return (
    <div className="preferences-panel">
      <div className="preferences-section">
        <div className="preferences-section-header">
          <Palette size={20} className="preferences-section-icon" />
          <div>
            <h3 className="preferences-section-title">Theme</h3>
            <p className="preferences-section-description">
              Choose a color theme for BoBe. This affects the avatar and all UI elements.
            </p>
          </div>
        </div>

        <div className="theme-grid">
          {themes.map((theme) => (
            <ThemeCard
              key={theme.id}
              theme={theme}
              isSelected={themeId === theme.id}
              onSelect={() => setTheme(theme.id)}
            />
          ))}
        </div>
      </div>
    </div>
  )
}

// =============================================================================
// THEME CARD
// =============================================================================

interface ThemeCardProps {
  theme: ThemeConfig
  isSelected: boolean
  onSelect: () => void
}

function ThemeCard({ theme, isSelected, onSelect }: ThemeCardProps) {
  return (
    <motion.button
      onClick={onSelect}
      className={cn('theme-card', isSelected && 'theme-card-selected')}
      whileHover={{ scale: 1.02 }}
      whileTap={{ scale: 0.98 }}
    >
      {/* Theme preview */}
      <div className="theme-preview" style={{ background: theme.colors.background }}>
        {/* Mini avatar preview */}
        <div className="theme-avatar-preview">
          <div
            className="theme-avatar-ring"
            style={{ background: theme.colors.surface, borderColor: theme.colors.border }}
          >
            <div
              className="theme-avatar-face"
              style={{
                background: `linear-gradient(145deg, ${theme.colors.avatarFaceLight} 0%, ${theme.colors.avatarFaceDark} 100%)`,
              }}
            >
              {/* Simple closed eyes for preview */}
              <div className="theme-avatar-eyes">
                <div
                  className="theme-avatar-eye-closed"
                  style={{ background: theme.colors.text }}
                />
                <div
                  className="theme-avatar-eye-closed"
                  style={{ background: theme.colors.text }}
                />
              </div>
            </div>
          </div>
        </div>

        {/* Color swatches */}
        <div className="theme-swatches">
          <div className="theme-swatch" style={{ background: theme.colors.primary }} />
          <div className="theme-swatch" style={{ background: theme.colors.secondary }} />
          <div className="theme-swatch" style={{ background: theme.colors.tertiary }} />
        </div>
      </div>

      {/* Theme info */}
      <div className="theme-info">
        <div className="theme-name-row">
          <span className="theme-name">{theme.name}</span>
          {isSelected && (
            <motion.div initial={{ scale: 0 }} animate={{ scale: 1 }} className="theme-check">
              <Check size={14} />
            </motion.div>
          )}
        </div>
      </div>
    </motion.button>
  )
}
