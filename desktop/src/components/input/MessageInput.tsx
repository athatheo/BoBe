/**
 * Message input component for user replies
 *
 * Compact input panel styled to match chat bubbles.
 * Uses Bauhaus-inspired design matching the rest of the UI.
 * Stays open after sending for continuous conversation.
 */

import { useState, useRef, useEffect } from 'react'
import { motion } from 'framer-motion'
import { Send, X } from 'lucide-react'
import { SPRING_CONFIG } from '@/lib/constants'

interface MessageInputProps {
  onSend: (message: string) => void
  onClose: () => void
  isThinking?: boolean
}

export function MessageInput({ onSend, onClose, isThinking = false }: MessageInputProps) {
  const [message, setMessage] = useState('')
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // Focus textarea on mount
  useEffect(() => {
    textareaRef.current?.focus()
  }, [])

  // Auto-resize textarea
  useEffect(() => {
    const textarea = textareaRef.current
    if (textarea) {
      textarea.style.height = 'auto'
      textarea.style.height = `${Math.min(textarea.scrollHeight, 40)}px`
    }
  }, [message])

  const handleSubmit = () => {
    const trimmed = message.trim()
    if (trimmed && !isThinking) {
      onSend(trimmed)
      setMessage('') // Clear input after send, but keep panel open
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSubmit()
    }
    if (e.key === 'Escape') {
      onClose()
    }
  }

  return (
    <motion.div
      className="message-input no-drag"
      initial={{ opacity: 0, y: 20, scale: 0.9 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 10, scale: 0.95 }}
      transition={{
        type: 'spring',
        ...SPRING_CONFIG,
      }}
    >
      <div className="message-input-container-compact">
        {/* Accent bar */}
        <div className="message-input-accent" />

        {/* Input area with close button */}
        <div className="message-input-content-compact">
          <textarea
            ref={textareaRef}
            className="message-input-textarea"
            placeholder={isThinking ? 'Draft a reply while thinking...' : 'Type a message...'}
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            onKeyDown={handleKeyDown}
            rows={1}
          />

          {/* Thinking hint - shown when user has typed but can't send yet */}
          {isThinking && message.trim() && (
            <span className="message-input-thinking-hint">waiting...</span>
          )}

          {/* Send button */}
          <motion.button
            className={`message-input-send ${message.trim() && !isThinking ? 'message-input-send-active' : 'message-input-send-disabled'}`}
            whileHover={message.trim() && !isThinking ? { scale: 1.05 } : {}}
            whileTap={message.trim() && !isThinking ? { scale: 0.95 } : {}}
            onClick={handleSubmit}
            disabled={!message.trim() || isThinking}
          >
            <Send size={14} strokeWidth={2.5} />
          </motion.button>

          {/* Close button */}
          <motion.button
            className="message-input-close-inline"
            whileTap={{ scale: 0.95 }}
            onClick={onClose}
          >
            <X size={14} strokeWidth={2.5} />
          </motion.button>
        </div>

        {/* Tail - points to avatar */}
        <div className="message-input-tail" />
      </div>
    </motion.div>
  )
}
