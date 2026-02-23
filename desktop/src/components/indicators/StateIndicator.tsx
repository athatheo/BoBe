/**
 * Unified state indicator component
 *
 * Uses minimalistic eyes to convey state - simple and expressive.
 */

import type { BobeStateType } from '@/types/bobe'
import { EyesIndicator } from './EyesIndicator'

interface StateIndicatorProps {
  state: BobeStateType
  chatOpen?: boolean
}

/**
 * Renders the eyes indicator based on the current state.
 * When chatOpen is true and state is idle, shows attentive (open) eyes.
 */
export function StateIndicator({ state, chatOpen = false }: StateIndicatorProps) {
  return (
    <div data-testid="state-indicator" data-state={state}>
      <EyesIndicator state={state} chatOpen={chatOpen} />
    </div>
  )
}
