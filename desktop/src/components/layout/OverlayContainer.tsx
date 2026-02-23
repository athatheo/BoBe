/**
 * Overlay container component
 *
 * Main layout wrapper for the overlay window.
 * Handles drag region and positioning.
 */

import type { ReactNode } from 'react'

interface OverlayContainerProps {
  children: ReactNode
}

/**
 * Container for the overlay window content.
 * Provides drag-to-move functionality and proper positioning.
 */
export function OverlayContainer({ children }: OverlayContainerProps) {
  return (
    <div className="drag-region w-full h-full flex flex-col items-end justify-end px-3">
      {children}
    </div>
  )
}
