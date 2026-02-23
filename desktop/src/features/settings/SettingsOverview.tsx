/**
 * SettingsOverview — friendly "How to change BoBe" landing page.
 *
 * Conversational guide using everyday language. Each section
 * explains what you can change in plain terms and links to
 * the relevant settings panel.
 */

import { motion } from 'framer-motion'
import { Eye, Brain, MessageCircle, Paintbrush, ArrowRight, Zap } from 'lucide-react'
import type { SettingsCategory } from '@/types/settings'

interface SettingsOverviewProps {
  onNavigate: (category: SettingsCategory) => void
}

export function SettingsOverview({ onNavigate }: SettingsOverviewProps) {
  return (
    <div className="ov">
      {/* Hero */}
      <motion.div
        className="ov-hero"
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3 }}
      >
        <h2 className="ov-title">How to change BoBe</h2>
        <p className="ov-subtitle">
          BoBe watches your screen, remembers what matters, and speaks up when it can help. Here's
          how to make it yours.
        </p>
      </motion.div>

      {/* Cards */}
      <div className="ov-cards">
        <Card
          icon={<Eye size={20} />}
          color="#C67B5C"
          heading="What BoBe sees"
          body="BoBe takes a look at your screen every few minutes to stay in the loop. You decide how often — or turn it off entirely."
          action="Screen capture settings"
          category="behavior"
          onNavigate={onNavigate}
          delay={0}
        />

        <Card
          icon={<Brain size={20} />}
          color="#8B9A7D"
          heading="What BoBe remembers"
          body="Short-term notes fade after a month. Long-term memories stick around. You can see everything BoBe has learned and delete anything."
          action="View memories"
          category="memories"
          onNavigate={onNavigate}
          delay={0.04}
        />

        <Card
          icon={<MessageCircle size={20} />}
          color="#D4A574"
          heading="When BoBe speaks up"
          body="BoBe checks in a few times a day — mornings, afternoons, evenings. Adjust the schedule, or set goals so BoBe knows what you care about."
          action="Check-in schedule"
          category="behavior"
          onNavigate={onNavigate}
          delay={0.08}
        />

        <Card
          icon={<Paintbrush size={20} />}
          color="#A69080"
          heading="How BoBe sounds"
          body="BoBe's personality comes from its Soul — a document that defines its tone, style, and character. Rewrite it to make BoBe sound like you want."
          action="Edit personality"
          category="souls"
          onNavigate={onNavigate}
          delay={0.12}
        />

        <Card
          icon={<Zap size={20} />}
          color="#8B9A7D"
          heading="What BoBe can do"
          body="BoBe can read files, search the web, check your browser history, and more. Enable or disable individual tools, or connect external servers via MCP."
          action="Manage tools"
          category="tools"
          onNavigate={onNavigate}
          delay={0.16}
        />
      </div>

      <p className="ov-footer">
        Use the sidebar to explore all settings. Everything runs locally on your Mac.
      </p>
    </div>
  )
}

// =============================================================================
// Card
// =============================================================================

function Card({
  icon,
  color,
  heading,
  body,
  action,
  category,
  onNavigate,
  delay,
}: {
  icon: React.ReactNode
  color: string
  heading: string
  body: string
  action: string
  category: SettingsCategory
  onNavigate: (c: SettingsCategory) => void
  delay: number
}) {
  return (
    <motion.button
      className="ov-card"
      onClick={() => onNavigate(category)}
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.25, delay }}
    >
      <span className="ov-card-icon" style={{ color }}>
        {icon}
      </span>
      <div className="ov-card-text">
        <span className="ov-card-heading">{heading}</span>
        <span className="ov-card-body">{body}</span>
      </div>
      <span className="ov-card-action" style={{ color }}>
        {action} <ArrowRight size={12} />
      </span>
    </motion.button>
  )
}
