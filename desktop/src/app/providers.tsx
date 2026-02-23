/**
 * Application providers
 *
 * ARCHITECTURE NOTE:
 * The BoBe state lives in an external store (bobe-store.ts), not in React context.
 * This provider exists for:
 * 1. Future features that need React context (themes, i18n, etc.)
 * 2. Error boundaries
 * 3. A clear composition root
 *
 * Components access state via useBobe() or useBobeSelector() directly,
 * NOT through context. This is intentional - external stores don't need
 * context for state distribution.
 */

import { type ReactNode } from 'react'

// =============================================================================
// PROVIDER
// =============================================================================

interface AppProvidersProps {
  children: ReactNode
}

/**
 * Application provider wrapper.
 *
 * Currently minimal - state is handled by external store.
 * Add providers here as needed (theme, i18n, etc.)
 */
export function AppProviders({ children }: AppProvidersProps) {
  return <>{children}</>
}
