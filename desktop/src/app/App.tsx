/**
 * Root application component
 *
 * ARCHITECTURE:
 * - State is managed by external store (bobe-store.ts)
 * - Components access state via useBobe() hook directly
 * - AppProviders wrapper is for future context needs (theme, i18n)
 */

import { AppProviders } from './providers'
import { OverlayWindow } from '@/features/overlay'

export function App() {
  return (
    <AppProviders>
      <OverlayWindow />
    </AppProviders>
  )
}
