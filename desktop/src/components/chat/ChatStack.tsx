/**
 * Chat stack component
 *
 * Container for chat bubbles with collapsible history.
 * Shows last message by default, expand to see full history.
 */

import { useState, useRef, useEffect } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { ChevronUp, ChevronDown } from 'lucide-react'
import type { ChatMessage } from '@/types/bobe'
import { ChatBubble } from './ChatBubble'

interface ChatStackProps {
  messages: ChatMessage[]
}

export function ChatStack({ messages }: ChatStackProps) {
  const [isExpanded, setIsExpanded] = useState(false)
  const stackRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to bottom when new messages arrive (in expanded mode)
  useEffect(() => {
    if (stackRef.current && isExpanded) {
      stackRef.current.scrollTop = stackRef.current.scrollHeight
    }
  }, [messages, isExpanded])

  if (messages.length === 0) {
    return null
  }

  // In collapsed mode, show the last 2 messages (user input + bobe reply)
  const visibleMessages = isExpanded ? messages : messages.slice(-2)
  const hiddenCount = Math.max(messages.length - 2, 0)

  return (
    <div className="chat-stack-wrapper">
      {/* Expand/Collapse button - only show if there's history */}
      {hiddenCount > 0 && (
        <motion.button
          className="chat-expand-button no-drag"
          onClick={() => setIsExpanded(!isExpanded)}
          whileHover={{ scale: 1.05 }}
          whileTap={{ scale: 0.95 }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
        >
          {isExpanded ? (
            <>
              <ChevronDown size={12} />
              <span>collapse</span>
            </>
          ) : (
            <>
              <ChevronUp size={12} />
              <span>+{hiddenCount} more</span>
            </>
          )}
        </motion.button>
      )}

      <div
        className={
          isExpanded ? 'chat-stack chat-stack-expanded' : 'chat-stack chat-stack-collapsed'
        }
        ref={stackRef}
      >
        <div className="chat-stack-content">
          <AnimatePresence mode="popLayout">
            {visibleMessages.map((message) => (
              <ChatBubble key={message.id} message={message} />
            ))}
          </AnimatePresence>
        </div>
      </div>
    </div>
  )
}
