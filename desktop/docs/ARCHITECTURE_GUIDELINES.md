# BoBe Shell - Architecture Guidelines

> Project structure, patterns, and communication architecture for Electron + React 19 + Tailwind 4

**Related docs:**

- [BEST_PRACTICES.md](./BEST_PRACTICES.md) - Security checklist, do/don't rules

---

## Table of Contents

1. [Project Structure](#project-structure)
2. [Process Model](#process-model)
3. [Communication Architecture](#communication-architecture)
4. [State Management](#state-management)
5. [React Patterns](#react-patterns)
6. [Loading & Error States](#loading--error-states)
7. [Component Organization](#component-organization)
8. [TypeScript Patterns](#typescript-patterns)
9. [Styling Architecture](#styling-architecture)
10. [Animation Patterns](#animation-patterns)

---

## Project Structure

```txt
bobe-shell/
├── electron/                     # Electron processes (main + preload)
│   ├── main/
│   │   └── index.ts             # App lifecycle, window creation
│   ├── preload/
│   │   └── index.ts             # Context bridge (window.bobe API)
│   ├── ipc/
│   │   ├── handlers.ts          # IPC handler implementations
│   │   └── index.ts             # Handler exports
│   ├── services/
│   │   ├── api-client.ts        # HTTP client for Python daemon
│   │   ├── event-sink.ts        # SSE connection (TODO)
│   │   ├── window-manager.ts    # Overlay window management
│   │   └── tray-manager.ts      # System tray
│   └── types/
│       └── index.ts             # Main process types
│
├── src/                          # React application
│   ├── app/
│   │   ├── App.tsx              # Root component
│   │   ├── providers.tsx        # Context providers (minimal)
│   │   └── index.tsx            # Entry point
│   ├── components/
│   │   ├── avatar/              # Avatar component system
│   │   ├── indicators/          # State indicators
│   │   │   ├── EyesIndicator.tsx   # Expressive eyes for all states
│   │   │   ├── IndicatorBubble.tsx # Bubble wrapper with effects
│   │   │   └── StateIndicator.tsx  # Unified state switch
│   │   ├── layout/              # Layout components
│   │   └── ui/                  # Radix-based primitives
│   ├── features/
│   │   └── overlay/             # Overlay feature module
│   ├── hooks/
│   │   ├── bobe-store.ts        # Single store: state + actions + hooks
│   │   ├── useAnimationState.ts # Animation coordination
│   │   └── index.ts             # Hook exports
│   ├── lib/
│   │   ├── cn.ts                # Tailwind class utility
│   │   └── constants.ts         # App constants
│   ├── types/
│   │   ├── bobe.ts              # Core types
│   │   ├── ipc.ts               # IPC contracts
│   │   └── window.d.ts          # Window augmentation
│   └── styles/
│       └── globals.css          # Tailwind + design tokens
│
├── electron-vite.config.ts      # Build configuration
├── tsconfig.json                # Base TypeScript config
├── tsconfig.node.json           # Node (electron) config
└── tsconfig.app.json            # App (renderer) config
```

### Key Decisions

1. **Separate `electron/` from `src/`** - Clear boundary between Electron and React
2. **No "renderer" terminology** - `src/` is a standard React app
3. **Feature-based organization** - Related files stay together
4. **Services encapsulate I/O** - HTTP, SSE, window management in services

---

## Process Model

Electron apps have three distinct execution contexts:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           MAIN PROCESS                                   │
│  - Full Node.js + Electron APIs                                         │
│  - Creates BrowserWindows                                                │
│  - Owns Python daemon relationship                                       │
│  - Manages app lifecycle                                                 │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │ IPC
                                │
┌───────────────────────────────▼─────────────────────────────────────────┐
│                          PRELOAD SCRIPT                                  │
│  - Runs in isolated context                                             │
│  - Bridge between main and renderer                                      │
│  - Exposes window.bobe API via contextBridge                            │
│  - Channel allowlisting for security                                     │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │ contextBridge
                                │
┌───────────────────────────────▼─────────────────────────────────────────┐
│                         RENDERER (React)                                 │
│  - Standard web environment                                              │
│  - No Node.js APIs                                                       │
│  - Accesses main via window.bobe                                         │
│  - Pure UI logic                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Trust Boundaries

| Process  | Trust Level  | Capabilities                         |
| -------- | ------------ | ------------------------------------ |
| Main     | Trusted      | Full OS access, file system, network |
| Preload  | Semi-trusted | IPC bridge only, minimal surface     |
| Renderer | Untrusted    | Web APIs only, sandboxed             |

---

## Communication Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Python Daemon                                │
│  llama.cpp + screen capture + proactive logic                       │
│                                                                      │
│  REST API:    POST /capture/start, POST /interrupt, GET /status     │
│  SSE Stream:  GET /events → state_change, proactive_message, token  │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ HTTP/SSE
                                   │
┌──────────────────────────────────▼──────────────────────────────────┐
│                      Electron Main Process                           │
│                                                                      │
│  ┌────────────────┐    ┌────────────────┐    ┌─────────────────┐   │
│  │  API Client    │    │  Event Sink    │    │  IPC Handlers   │   │
│  │  (REST calls)  │    │  (SSE listen)  │    │  (invoke/handle)│   │
│  └────────────────┘    └────────────────┘    └─────────────────┘   │
│                                                      │               │
└──────────────────────────────────────────────────────┼───────────────┘
                                                       │ IPC
┌──────────────────────────────────────────────────────▼───────────────┐
│                         Renderer (React)                              │
│                                                                       │
│  window.bobe.toggleCapture() → IPC invoke → Main → HTTP → Python     │
│  window.bobe.onStateUpdate() ← IPC send ← Main ← SSE ← Python        │
└───────────────────────────────────────────────────────────────────────┘
```

### Why IPC Through Main (Not Direct HTTP from Renderer)

| Direct from Renderer           | Through Main Process   |
| ------------------------------ | ---------------------- |
| Renderer knows daemon details  | Renderer is pure UI    |
| Two error handling paths       | Centralized handling   |
| Can't manage daemon lifecycle  | Main owns relationship |
| Connection state in two places | Single source of truth |

### Two Data Flows

| Flow         | Direction                | Pattern                | Use Case      |
| ------------ | ------------------------ | ---------------------- | ------------- |
| **Commands** | Renderer → Main → Python | IPC invoke → HTTP POST | User actions  |
| **Events**   | Python → Main → Renderer | SSE → IPC send         | State updates |

### IPC Channel Contracts

```typescript
// Commands (invoke/handle)
'bobe:get-state'        → BobeState
'bobe:toggle-capture'   → boolean
'bobe:dismiss-message'  → void
'bobe:resize-for-bubble' → void

// Events (send/on)
'bobe:state-update'     → BobeState
```

---

## State Management

### Single Store Architecture (React 2026 Pattern)

We use a **single external store** with `useSyncExternalStore`. This avoids hook nesting, prevents dependency hell, and keeps actions as plain functions.

**Key Principles:**

1. **No hook nesting** - Single `useBobe()` hook, no composition
2. **Actions are functions, not hooks** - Call from anywhere, no per-instance state
3. **Local loading via `useTransition`** - React 19 pattern for pending states
4. **Selector hook for performance** - `useBobeSelector()` for fine-grained subscriptions

```typescript
// hooks/bobe-store.ts - Everything in one file

// Store primitives (not React)
let currentState: BobeContext = DEFAULT_CONTEXT
const listeners = new Set<() => void>()

function getSnapshot() { return currentState }
function subscribe(cb: () => void) {
  listeners.add(cb)
  return () => listeners.delete(cb)
}
function setState(partial: Partial<BobeContext>) {
  currentState = { ...currentState, ...partial, stateType: deriveStateType(...) }
  listeners.forEach(cb => cb())
}

// Actions - plain async functions, NOT hooks
async function toggleCapture(): Promise<boolean | undefined> {
  return window.bobe?.toggleCapture()
}

export const bobeActions = { toggleCapture, dismissMessage, resizeForBubble } as const

// Single hook - full state + actions
export function useBobe() {
  const state = useSyncExternalStore(subscribe, getSnapshot, getSnapshot)
  return { state, ...bobeActions }
}

// Selector hook - performance optimization
export function useBobeSelector<T>(selector: (state: BobeContext) => T): T {
  return useSyncExternalStore(
    subscribe,
    () => selector(getSnapshot()),
    () => selector(getSnapshot())
  )
}
```

### Hook Usage

| Hook                                | Use Case                                | Re-renders                       |
| ----------------------------------- | --------------------------------------- | -------------------------------- |
| `useBobe()`                         | Most components needing state + actions | On any state change              |
| `useBobeSelector(s => s.capturing)` | Performance-critical, need one field    | Only when selected value changes |

**Why no hook composition?**

- Hook nesting creates hidden dependencies
- Per-hook state (like `isLoading`) causes inconsistencies across instances
- Flat architecture is easier to reason about and debug

### State Derivation

UI state is derived from context, not stored separately:

```typescript
// types/bobe.ts
export type BobeStateType =
  | 'loading'
  | 'idle'
  | 'capturing'
  | 'thinking'
  | 'speaking'
  | 'wants_to_speak'

export function deriveStateType(context: BobeContext): BobeStateType {
  if (!context.daemonConnected) return 'loading'
  if (context.speaking) return 'speaking'
  if (context.thinking) return 'thinking'
  if (context.lastMessage && !context.speaking) return 'wants_to_speak'
  if (context.capturing) return 'capturing'
  return 'idle'
}
```

---

## React Patterns

### Component Design

```tsx
// Prefer: Props for configuration, context for state
function Avatar({ onClick }: { onClick?: () => void }) {
  const { state, toggleCapture } = useBobe()
  return (
    <div onClick={onClick}>
      <StateIndicator state={state.stateType} />
    </div>
  )
}

// Avoid: Prop drilling state
function Avatar({ capturing, thinking, speaking, onToggle }: ...) { ... }
```

### Exhaustive Switch Pattern

```tsx
function StateIndicator({ state }: { state: BobeStateType }) {
  switch (state) {
    case 'loading':
      return <LoadingVariant />
    case 'idle':
      return <IdleVariant />
    case 'capturing':
      return <CapturingVariant />
    case 'thinking':
      return <ThinkingVariant />
    case 'speaking':
      return <SpeakingVariant />
    case 'wants_to_speak':
      return <WantsToSpeakVariant />
    default: {
      const _exhaustive: never = state // TypeScript enforces completeness
      return null
    }
  }
}
```

---

## Loading & Error States

### Connection States

```typescript
type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'error'

// Show appropriate UI for each state
function ConnectionIndicator() {
  const isConnected = useBobeSelector(s => s.daemonConnected)

  if (!isConnected) {
    return <LoadingIndicator message="Connecting to BoBe..." />
  }
  return null
}
```

### Action Loading States (React 19 Pattern)

Use `useTransition` for local pending states instead of global loading flags:

```tsx
import { useTransition } from 'react'

function CaptureButton() {
  const { toggleCapture } = useBobe()
  const [isPending, startTransition] = useTransition()

  return (
    <button onClick={() => startTransition(() => toggleCapture())} disabled={isPending}>
      {isPending ? <Spinner /> : 'Toggle Capture'}
    </button>
  )
}
```

**Why `useTransition` over hook state?**

- Loading state is local to the component, not global
- No stale closure issues
- Works with React's concurrent features
- Each button instance has independent pending state

### Error Boundaries

```tsx
// app/ErrorBoundary.tsx
class ErrorBoundary extends Component<Props, State> {
  state = { hasError: false, error: null }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error }
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="error-fallback">
          <h2>Something went wrong</h2>
          <button onClick={() => this.setState({ hasError: false })}>Try again</button>
        </div>
      )
    }
    return this.props.children
  }
}

// Usage in App.tsx
;<ErrorBoundary>
  <AppProviders>
    <OverlayWindow />
  </AppProviders>
</ErrorBoundary>
```

### Skeleton Loading Pattern

```tsx
// For content that loads asynchronously
function MessageContent({ message }: { message: string | null }) {
  if (message === null) {
    return <MessageSkeleton />
  }
  return <p>{message}</p>
}

function MessageSkeleton() {
  return (
    <div className="animate-pulse">
      <div className="h-4 bg-gray-200 rounded w-3/4 mb-2" />
      <div className="h-4 bg-gray-200 rounded w-1/2" />
    </div>
  )
}
```

### Timeout Handling

```tsx
function useWithTimeout<T>(promise: Promise<T>, timeoutMs = 5000): Promise<T> {
  return Promise.race([
    promise,
    new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error('Request timed out')), timeoutMs),
    ),
  ])
}
```

---

## Component Organization

### Directory Structure

```
components/
├── avatar/
│   ├── Avatar.tsx           # Main export
│   └── index.ts             # Re-exports
├── indicators/
│   ├── EyesIndicator.tsx    # All eye states in one file
│   ├── IndicatorBubble.tsx  # Bubble wrapper with ring effects
│   ├── StateIndicator.tsx   # Switch component (uses EyesIndicator)
│   └── index.ts
└── ui/                      # Radix primitives
    ├── button.tsx
    ├── dropdown.tsx
    └── tooltip.tsx
```

### Naming Conventions

| Type        | Convention           | Example                             |
| ----------- | -------------------- | ----------------------------------- |
| Components  | PascalCase           | `Avatar.tsx`, `StateIndicator.tsx`  |
| Store files | kebab-case           | `bobe-store.ts`                     |
| Hooks       | camelCase with `use` | `useBobe`, `useBobeSelector`        |
| Types       | PascalCase           | `BobeContext`, `BobeStateType`      |
| Constants   | SCREAMING_SNAKE      | `SPRING_CONFIG`, `INDICATOR_TIMING` |
| CSS files   | kebab-case           | `globals.css`                       |
| Directories | kebab-case           | `state-indicator/`                  |

---

## TypeScript Patterns

### Exhaustive Switches on Union Types

```typescript
// BobeStateType is a string union; exhaustive switch ensures all cases handled
type BobeStateType = 'loading' | 'idle' | 'capturing' | 'thinking' | 'speaking' | 'wants_to_speak'

function renderForState(state: BobeStateType) {
  switch (state) {
    case 'loading': return <LoadingEyes />
    case 'idle': return <SleepingEyes />
    case 'capturing': return <CapturingEyes />
    case 'thinking': return <ThinkingEyes />
    case 'speaking': return <SpeakingEyes />
    case 'wants_to_speak': return <EagerEyes />
    default: {
      const _exhaustive: never = state
      return null
    }
  }
}
```

### IPC Type Safety

```typescript
// types/ipc.ts
export const IPC_CHANNELS = {
  GET_STATE: 'bobe:get-state',
  TOGGLE_CAPTURE: 'bobe:toggle-capture',
  STATE_UPDATE: 'bobe:state-update',
} as const

export type IpcChannel = (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS]
```

---

## Styling Architecture

### Tailwind 4 with @theme

```css
/* globals.css */
@import 'tailwindcss';

@theme {
  /* Design tokens */
  --color-bobe-terracotta: #c67b5c;
  --color-bobe-sand: #e8dcc4;
  --avatar-outer: 116px;
  --shadow-avatar: 0 4px 20px rgba(58, 58, 58, 0.12);
}

/* Component classes using tokens */
.avatar-card {
  @apply relative flex items-center justify-center rounded-full;
  width: var(--avatar-outer);
  background: var(--color-bobe-warm-white);
  box-shadow: var(--shadow-avatar);
}
```

### Class Composition

```tsx
import { cn } from '@/lib/cn'

// Always use cn() for conditional classes
;<div
  className={cn(
    'base-classes',
    isActive && 'active-classes',
    className, // Allow override from props
  )}
/>
```

---

## Animation Patterns

### Framer Motion Guidelines

```tsx
// Spring animations for UI feedback
<motion.div
  whileHover={{ scale: 1.06 }}
  whileTap={{ scale: 0.96 }}
  transition={{ type: 'spring', stiffness: 300, damping: 20 }}
/>

// Presence animations for mount/unmount
<AnimatePresence mode="wait">
  {isVisible && (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -10 }}
    />
  )}
</AnimatePresence>
```

### Performance Rules

1. Animate `transform` and `opacity` only (GPU-accelerated)
2. Use `motion.div` not animated state in React
3. Keep animation duration under 300ms for UI feedback
4. Respect `prefers-reduced-motion`

---

## Summary

| Principle             | Implementation                                     |
| --------------------- | -------------------------------------------------- |
| Process isolation     | Main/preload/renderer separation                   |
| IPC through main      | All daemon communication via main process          |
| Single external store | `useSyncExternalStore` in `bobe-store.ts`          |
| No hook nesting       | One hook (`useBobe`), actions as plain functions   |
| Local loading state   | React 19 `useTransition` pattern                   |
| Selector for perf     | `useBobeSelector()` for fine-grained subscriptions |
| Exhaustive types      | Discriminated unions + switch                      |
| Tailwind tokens       | @theme for design system                           |
| Semantic CSS          | Component classes over inline styles               |
