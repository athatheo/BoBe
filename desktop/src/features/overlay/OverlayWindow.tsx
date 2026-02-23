/**
 * Overlay window feature
 *
 * Main feature component that orchestrates the overlay UI.
 * Manages chat panel visibility, indicator bubbles, and avatar state.
 */

import { useLayoutEffect, useState, useEffect, useRef } from 'react'
import { AnimatePresence } from 'framer-motion'
import {
  Avatar,
  MessageInput,
  OverlayContainer,
  ChatStack,
  IndicatorBubble,
} from '@/components'
import { useBobe, useBobeSelector, bobeActions } from '@/hooks'
import { WINDOW_SIZES } from '@/lib/constants'

// Auto-close chat after 10 minutes of no message activity while idle
const INACTIVITY_TIMEOUT_MS = 10 * 60 * 1000
const INACTIVITY_CHECK_INTERVAL_MS = 30_000

/**
 * Calculate window dimensions based on content.
 */
function calculateWindowSize(showChat: boolean, messageCount: number) {
  if (!showChat) {
    // Collapsed - just avatar, regardless of message count
    return {
      width: WINDOW_SIZES.WIDTH_COLLAPSED,
      height: WINDOW_SIZES.HEIGHT_COLLAPSED,
    }
  }

  // Expanded - calculate based on content
  const width: number = WINDOW_SIZES.WIDTH_EXPANDED

  // Base: avatar area (includes padding and label overflow)
  let height: number = WINDOW_SIZES.HEIGHT_AVATAR

  // Add input panel
  height += WINDOW_SIZES.HEIGHT_INPUT

  // Show up to 2 messages in collapsed view (user + reply)
  const visibleMessages = Math.min(messageCount, 2)
  height += visibleMessages * WINDOW_SIZES.HEIGHT_MESSAGE

  // Clamp to max
  height = Math.min(height, WINDOW_SIZES.HEIGHT_MAX)

  return { width, height }
}

export function OverlayWindow() {
  const { state, toggleCapture, sendMessage, messages } = useBobe()
  // useBobeSelector for display-only values — only re-renders when these specific slices change
  const activeIndicator = useBobeSelector((s) => s.activeIndicator)
  const toolExecutions = useBobeSelector((s) => s.toolExecutions)
  const [showChat, setShowChat] = useState(false)

  // Track previous message count for auto-open detection
  const prevMessagesLengthRef = useRef(messages.length)

  // Track last message-based interaction for inactivity auto-close
  const lastMessageActivityRef = useRef(Date.now())

  // Auto-open chat when new bobe message arrives (proactive check-in)
  useEffect(() => {
    const prevCount = prevMessagesLengthRef.current
    prevMessagesLengthRef.current = messages.length

    // If new message appeared and chat is closed
    if (messages.length > prevCount && !showChat) {
      // Check if the newest message is from bobe (not user)
      const newestMessage = messages[messages.length - 1]
      if (newestMessage?.sender === 'bobe') {
        setShowChat(true)
      }
    }
  }, [messages, showChat])

  // Reset inactivity timer when messages change (new message sent or received)
  useEffect(() => {
    lastMessageActivityRef.current = Date.now()
  }, [messages.length])

  // Auto-close chat after 10 minutes of no message activity while idle
  useEffect(() => {
    if (!showChat || state.stateType !== 'idle') return

    const checkInterval = setInterval(() => {
      const elapsed = Date.now() - lastMessageActivityRef.current
      if (elapsed >= INACTIVITY_TIMEOUT_MS) {
        setShowChat(false)
      }
    }, INACTIVITY_CHECK_INTERVAL_MS)

    return () => clearInterval(checkInterval)
  }, [showChat, state.stateType])

  // Resize window dynamically based on content
  // Using useLayoutEffect for synchronous resize before paint
  // Note: bobeActions.resizeWindow is a stable module-level reference
  useLayoutEffect(() => {
    const { width, height } = calculateWindowSize(showChat, messages.length)
    bobeActions.resizeWindow(width, height)
  }, [showChat, messages.length])

  const handleToggleChat = () => {
    if (showChat) {
      // Closing chat - optionally clear messages
      setShowChat(false)
    } else {
      setShowChat(true)
    }
  }

  const handleSendMessage = async (content: string) => {
    lastMessageActivityRef.current = Date.now()
    try {
      await sendMessage(content)
      // Input stays open for continuous conversation
    } catch (error) {
      console.error('Failed to send message:', error)
    }
  }

  const handleCloseChat = () => {
    setShowChat(false)
    // Optionally clear messages when closing
    // clearMessages()
  }

  // Show message badge if there are messages but chat is closed
  const hasUnreadMessages = messages.length > 0 && !showChat

  return (
    <OverlayContainer>
      {/* Chat stack - messages above input */}
      {showChat && <ChatStack messages={messages} />}

      {/* Input panel - stays at bottom when chat is open */}
      <AnimatePresence>
        {showChat && (
          <div className="flex w-full flex-col gap-1">
            <MessageInput
              onSend={handleSendMessage}
              onClose={handleCloseChat}
              isThinking={state.thinking}
            />
          </div>
        )}
      </AnimatePresence>

      {/* Avatar with indicator bubble */}
      {/* Note: thinking/analyzing show via StatusLabel on avatar, not here */}
      <div className={`avatar-with-indicator ${showChat ? 'mt-1' : ''}`}>
        <IndicatorBubble
          indicator={
            activeIndicator === 'thinking' || activeIndicator === 'analyzing'
              ? null
              : activeIndicator
          }
          toolExecutions={toolExecutions}
        />
        <Avatar
          stateType={state.stateType}
          isCapturing={state.capturing}
          isConnected={state.daemonConnected}
          hasMessage={hasUnreadMessages}
          showInput={showChat}
          onClick={handleToggleChat}
          onToggleCapture={toggleCapture}
          onToggleInput={handleToggleChat}
        />
      </div>
    </OverlayContainer>
  )
}
