/**
 * Chat bubble component
 *
 * Individual message bubble in the chat stack.
 * Supports both user and BoBe messages with different styling.
 * Includes streaming animation for in-progress responses.
 */

import { motion } from 'framer-motion'
import type { ChatMessage } from '@/types/bobe'
import { cn } from '@/lib/cn'
import { SPRING_CONFIG } from '@/lib/constants'

interface ChatBubbleProps {
  message: ChatMessage
}

export function ChatBubble({ message }: ChatBubbleProps) {
  const isUser = message.sender === 'user'
  const isPending = message.isPending

  return (
    <motion.div
      className={cn(
        'chat-bubble no-drag',
        isUser ? 'chat-bubble-user' : 'chat-bubble-bobe',
        isPending && 'chat-bubble-pending',
      )}
      initial={{ opacity: 0, y: 20, scale: 0.95 }}
      animate={{ opacity: isPending ? 0.5 : 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.95 }}
      transition={{
        type: 'spring',
        ...SPRING_CONFIG,
      }}
      layout
    >
      <div
        className={cn(
          'chat-bubble-container',
          isUser ? 'chat-bubble-container-user' : 'chat-bubble-container-bobe',
          isPending && 'chat-bubble-container-pending',
        )}
      >
        {/* Accent bar */}
        <div
          className={cn(
            'chat-bubble-accent',
            isUser ? 'chat-bubble-accent-user' : 'chat-bubble-accent-bobe',
          )}
        />

        {/* Content */}
        <div className="chat-bubble-content">
          <span
            className={cn(
              'chat-bubble-sender',
              isUser ? 'chat-bubble-sender-user' : 'chat-bubble-sender-bobe',
            )}
          >
            {isUser ? 'you' : 'bobe'}
            {isPending && <span className="chat-bubble-pending-label"> - sending...</span>}
          </span>
          <p className="chat-bubble-message">
            {message.content}
            {message.isStreaming && <StreamingCursor />}
          </p>
        </div>
      </div>
    </motion.div>
  )
}

function StreamingCursor() {
  return (
    <motion.span
      className="chat-bubble-cursor"
      animate={{ opacity: [1, 0] }}
      transition={{ duration: 0.6, repeat: Infinity, repeatType: 'reverse' }}
    >
      |
    </motion.span>
  )
}
