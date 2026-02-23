# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Local Terminal & CLI Tools

This repo is commonly worked on in a macOS + fish environment with non-default shell behaviors (aliases, fish scripting differences) and additional CLI tools.

See `docs/TerminalAndCLITools.instructions.md` for:

- Fish-specific scripting dos/don’ts (e.g., alias bypassing, bash fallback)
- Agent-relevant CLI tools available on the machine (e.g., `jq`, `xh`, `gh`, etc)

## Project Overview

**BoBe Desktop** is a modern Electron + React desktop overlay application serving as the UI for a local, proactive AI companion. It's a **thin client** that communicates with a Python backend daemon via HTTP/SSE—all business logic, screen capture, LLM inference, and state management happens in the Python daemon.

## Build & Development Commands

```bash
pnpm dev          # Start Electron with hot reload (Vite dev server)
pnpm build        # Production build (outputs to out/)
pnpm preview      # Preview production build
pnpm typecheck    # Run both node and app type checks
```

## Testing

### E2E Tests (Playwright + Electron)

```bash
pnpm test:e2e         # Build app and run all E2E tests
pnpm test:e2e:headed  # Run with visible Electron window
pnpm test:e2e:debug   # Run in Playwright debug mode
pnpm test:e2e:ui      # Run with Playwright UI
```

**Test structure:**

```
e2e/
├── fixtures/
│   └── electron.fixture.ts   # Reusable Electron app launcher
├── tests/
│   ├── app.spec.ts           # Main process, security, IPC tests
│   └── overlay.spec.ts       # React UI tests (avatar, indicators)
└── tsconfig.json
```

**Writing tests:**

```typescript
import { test, expect } from '../fixtures/electron.fixture'

test('example test', async ({ electronApp, window }) => {
  // electronApp: Electron main process access
  // window: Playwright Page for React UI (like browser testing)

  await expect(window.locator('[data-testid="avatar"]')).toBeVisible()
})
```

### Browser Mode (Development/Testing without Electron)

The React app can run standalone in a browser, connecting directly to the Python daemon via HTTP/SSE. This is enabled by `src/lib/browser-daemon-client.ts`.

**How it works:**

- In Electron: Uses `window.bobe` from preload script (IPC)
- In Browser: Uses `BrowserDaemonClient` (direct HTTP/SSE to daemon)

**Use cases:**

- Faster UI iteration (no Electron startup)
- Testing with Playwright MCP
- Debugging React components in browser DevTools

**To use:**

1. Start dev server: `pnpm dev`
2. Open `http://localhost:5173` in browser (ignore Electron window)
3. If daemon is running on `localhost:8766`, UI will connect automatically

### Playwright MCP (Interactive Testing)

Claude Code has access to a Playwright MCP for interactive browser testing during conversations.

**What it can do:**

- Navigate to the dev server and take screenshots
- Click elements, fill forms, interact with the UI
- Capture console logs and errors
- Visual inspection without running full E2E suite

**When to use:**

- Quick visual checks: "Show me what the avatar looks like"
- Debugging UI issues: "Click the chat button and screenshot"
- Exploring interactions: "What happens when I click X?"

**Limitations:**

- Browser only (no Electron shell, no `window.bobe` from preload)
- Needs dev server running (`pnpm dev`)
- For full Electron testing, use `pnpm test:e2e`

### Test Data Attributes

Components have `data-testid` attributes for reliable test selectors:

| Selector                                         | Component               |
| ------------------------------------------------ | ----------------------- |
| `[data-testid="avatar"]`                         | Main avatar container   |
| `[data-testid="state-indicator"]`                | State indicator wrapper |
| `[data-testid="state-indicator-loading"]`        | Loading eyes            |
| `[data-testid="state-indicator-idle"]`           | Sleeping eyes           |
| `[data-testid="state-indicator-capturing"]`      | Capturing eyes          |
| `[data-testid="state-indicator-thinking"]`       | Thinking eyes           |
| `[data-testid="state-indicator-speaking"]`       | Speaking eyes           |
| `[data-testid="state-indicator-wants_to_speak"]` | Eager eyes              |

## Tech Stack

- **Electron 40** with electron-vite for bundling
- **React 19** + TypeScript 5
- **Tailwind CSS 4** (using `@import "tailwindcss"` syntax with `@theme` for design tokens)
- **Framer Motion** for animations
- **Radix UI** primitives (dropdown-menu, popover, switch, tooltip)
- **Playwright** for E2E testing

## Project Structure

```txt
bobe-desktop/
├── docs/                          # Architecture, Best Practices, instructions
├── e2e/                           # Playwright E2E tests
│   ├── fixtures/                 # Test fixtures (electron.fixture.ts)
│   └── tests/                    # Test specs (app.spec.ts, overlay.spec.ts)
├── electron/                      # Electron main process code
│   ├── main/                     # Main process entry
│   │   └── index.ts
│   ├── preload/                  # Preload script (context bridge)
│   │   └── index.ts
│   ├── ipc/                      # IPC handlers
│   │   ├── handlers.ts
│   │   └── index.ts
│   ├── services/                 # Main process services
│   │   ├── api-client.ts        # HTTP client for Python daemon
│   │   ├── window-manager.ts    # Overlay window management
│   │   ├── tray-manager.ts      # System tray management
│   │   └── index.ts
│   └── types/                    # Main process types
│       └── index.ts
│
├── src/                          # React application (pure web)
│   ├── app/                      # Application root
│   │   ├── App.tsx              # Root component
│   │   ├── providers.tsx        # Context providers (AppProviders)
│   │   └── index.tsx            # Entry point
│   │
│   ├── components/               # Reusable components
│   │   ├── avatar/              # Avatar component system
│   │   │   └── Avatar.tsx
│   │   ├── indicators/          # Status indicators
│   │   │   ├── EyesIndicator.tsx   # Expressive eyes for all states
│   │   │   ├── IndicatorBubble.tsx # Bubble wrapper with effects
│   │   │   ├── StateIndicator.tsx  # Unified state switch
│   │   │   └── index.ts            # Exports
│   │   ├── layout/              # Layout components
│   │   │   └── OverlayContainer.tsx
│   │   └── ui/                  # Radix-based UI primitives (future)
│   │
│   ├── features/                 # Feature modules
│   │   └── overlay/             # Overlay feature
│   │       └── OverlayWindow.tsx
│   │
│   ├── hooks/                    # Shared React hooks
│   │   ├── bobe-store.ts        # Single store: state + actions + hooks
│   │   ├── useAnimationState.ts # Animation coordination
│   │   └── index.ts             # Exports
│   │
│   ├── lib/                      # Utilities & helpers
│   │   ├── cn.ts                # Tailwind merge utility
│   │   ├── constants.ts         # Animation/layout constants
│   │   └── browser-daemon-client.ts  # Direct daemon client for browser mode
│   │
│   ├── types/                    # Shared TypeScript types
│   │   ├── bobe.ts              # Core types with discriminated unions
│   │   ├── ipc.ts               # IPC contracts
│   │   └── window.d.ts          # Window augmentation
│   │
│   ├── styles/                   # Global styles
│   │   └── globals.css          # Tailwind + design tokens
│   │
│   └── index.html               # HTML entry point
│
├── electron-vite.config.ts      # Electron Vite config
├── tsconfig.json                # Root TypeScript config
├── tsconfig.node.json           # Electron TypeScript config
├── tsconfig.app.json            # React app TypeScript config
└── package.json
```

## Architecture

### Three-Process Model

1. **Main Process** (`electron/main/`) - Creates overlay window, manages tray, broadcasts state via IPC
2. **Preload Script** (`electron/preload/`) - Context bridge exposing `window.bobe` API with channel allowlisting
3. **Renderer** (`src/`) - Pure React UI, consumes state from preload bridge, no direct Electron imports

### Critical Architecture Decision: IPC Through Main Process

All Python daemon communication must route through the main process:

```
Renderer → IPC → Main Process → HTTP/SSE → Python Daemon
```

The renderer never talks to the daemon directly. This:

- Keeps renderer as a pure web app (security best practice)
- Centralizes connection management and error handling
- Allows main to manage daemon lifecycle

### State Flow

```
Python Daemon → Main Process → (IPC broadcast) → Renderer → Avatar + Indicators + Speech Bubble
```

State is managed via `useSyncExternalStore` pattern:

- Single source of truth in main process
- IPC events update renderer store
- Components subscribe via hooks:
  - `useBobe()` - Full access (state + actions) - use for most components
  - `useBobeSelector(selector)` - Subscribe to state slice - use for performance optimization

### State Types (Discriminated Unions)

```typescript
type BobeStateType = 'loading' | 'idle' | 'capturing' | 'thinking' | 'speaking' | 'wants_to_speak'
```

Priority order for state display: loading > speaking > thinking > wants_to_speak > capturing > idle

### Component Structure

```
App (AppProviders)
└── OverlayWindow (feature module)
    └── Avatar
        ├── StateIndicator → EyesIndicator (expressive eyes)
        ├── ThinkingNumbers / SpeakingWave (ring effects)
        ├── ConnectionDot
        ├── MessageBadge
        └── CaptureToggle
```

### Overlay Window Configuration

The overlay is frameless, transparent, always-on-top:

```typescript
{
  frame: false,
  transparent: true,
  alwaysOnTop: true,
  skipTaskbar: true,
  webPreferences: {
    contextIsolation: true,
    nodeIntegration: false,
    sandbox: true
  }
}
```

## Key Patterns

### IPC Pattern

Handler-based (main process):

```typescript
ipcMain.handle('bobe:get-state', () => getState())
```

Event broadcast (main → renderer):

```typescript
overlayWindow.webContents.send('bobe:state-update', stateCopy)
```

Renderer subscription:

```typescript
const unsubscribe = window.bobe.onStateUpdate((state) => setState(state))
```

### Preload API (`window.bobe`)

- `getState()` - Get current state
- `toggleCapture()` - Toggle screen capture
- `dismissMessage()` - Dismiss current message
- `resizeForBubble(show)` - Resize window for speech bubble
- `onStateUpdate(callback)` - Subscribe to state changes

### Preload Security

- Explicit channel allowlisting via `INVOKE_CHANNELS` and `EVENT_CHANNELS` arrays
- Callbacks wrapped to strip IPC event objects
- Never exposes raw `ipcRenderer` methods

### StateIndicator Pattern

Uses `EyesIndicator` with exhaustive switch for type-safe state rendering:

```typescript
// EyesIndicator.tsx
switch (state) {
  case 'loading': return <LoadingEyes />
  case 'idle': return <SleepingEyes />
  case 'capturing': return <CapturingEyes />
  case 'thinking': return <ThinkingEyes />
  case 'speaking': return <SpeakingEyes />
  case 'wants_to_speak': return <EagerEyes />
  default: const _exhaustive: never = state
}
```

### Design System

Bauhaus-inspired color palette defined as CSS variables in `globals.css`:

- `--bobe-terracotta`: #C67B5C (accent)
- `--bobe-sand`: #E8DCC4 (neutral)
- `--bobe-warm-white`: #FAF7F2 (background)
- `--bobe-olive`: #8B9A7D (secondary)
- `--bobe-charcoal`: #3A3A3A (text)
- `--bobe-clay`: #A69080 (tertiary)

Typography: SF Pro Rounded (macOS native)

### Animation Constants

All animation values are in `src/lib/constants.ts`:

- `ANIMATION.DURATION.*` - Timing values (FAST, NORMAL, SLOW, etc.)
- `ANIMATION.SPRING.*` - Spring physics (STIFFNESS, DAMPING, MASS)
- `OVERLAY.*` - Window/avatar dimensions

### Animation Patterns

- Use Framer Motion for all animations
- Breathing animations (idle): 3.5s cycle, scale + opacity
- Spring transitions (speech bubble): damping 20, stiffness 300
- AnimatePresence for mount/unmount animations
- Respect `prefers-reduced-motion`

### Tailwind Usage

- Use `cn()` utility from `lib/cn.ts` for className composition
- Use `@theme` block in globals.css for design tokens (Tailwind 4)
- Electron-specific: `.drag-region` for window dragging, `.no-drag` for interactive elements

## Security Architecture

All security hardening lives in `electron/services/security.ts`. See the [Electron Security Checklist](https://www.electronjs.org/docs/latest/tutorial/security).

### Why We Do Each Thing

| Measure                             | Why                                                                                                                                                                           | File                 |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| **Custom `app://` protocol**        | `file://` gives renderers extra OS-level privileges and silently ignores CSP headers. `app://` behaves like HTTPS (proper origin model, CSP works).                           | `security.ts`        |
| **CSP via `onHeadersReceived`**     | Blocks XSS — only scripts from `app://bobe` can execute. `default-src 'none'` deny-all baseline. `connect-src` locked to `127.0.0.1:8766` only.                               | `security.ts`        |
| **`app.isPackaged` not `NODE_ENV`** | `NODE_ENV` is a regular env var any process can set. `app.isPackaged` is determined by Electron at build time (whether app runs from asar) and can't be tampered.             | `security.ts`        |
| **Navigation guards**               | Prevents a compromised renderer from navigating to attacker-controlled pages. Blocks all navigation + popup windows globally via `web-contents-created`.                      | `security.ts`        |
| **Permission handler (deny all)**   | Without this, Electron **defaults to allowing** camera/mic/geolocation requests from any renderer. We deny all by default.                                                    | `security.ts`        |
| **Certificate error handler**       | Blocks MITM attacks — rejects invalid TLS certs in production.                                                                                                                | `security.ts`        |
| **Electron fuses**                  | Build-time binary flags that can't be changed at runtime. Disables `ELECTRON_RUN_AS_NODE`, `NODE_OPTIONS` injection, `file://` privileges. Enables asar integrity validation. | `build/afterPack.js` |
| **Directory traversal protection**  | Protocol handler validates `resolvedPath.startsWith(RENDERER_DIR)` to prevent `../` path escapes.                                                                             | `security.ts`        |

### Renderer Process Rules

- **Never** enable `nodeIntegration` in renderer
- **Always** use `contextIsolation: true`, `sandbox: true`
- **Always** set `webSecurity: true`, `allowRunningInsecureContent: false` explicitly
- Preload script allowlists specific IPC channels (73 channels, no wildcards)
- Wrap callbacks to strip IPC event objects
- Validate all inputs in main process IPC handlers (`settings-handlers.ts`)

### Dev vs Production CSP

| Directive     | Production              | Dev                                    | Why dev differs                         |
| ------------- | ----------------------- | -------------------------------------- | --------------------------------------- |
| `script-src`  | `app://bobe`            | `'self' 'unsafe-inline' 'unsafe-eval'` | Vite HMR requires eval + inline scripts |
| `worker-src`  | `app://bobe blob:`      | `'self' blob:`                         | Vite creates blob workers               |
| `connect-src` | `http://127.0.0.1:8766` | `ws://localhost:* http://localhost:*`  | Vite HMR websocket                      |

The Electron dev console warning about "Insecure CSP" is expected — it fires because dev CSP includes `unsafe-eval`. This goes away in packaged builds.

### Build Pipeline Security

```
pnpm dist → electron-vite build → electron-builder pack
  → build/afterPack.js (flip fuses on binary)
  → macOS code signing
  → build/notarize.js (Apple notarization)
  → .dmg output
```

### Supply Chain Protection

- `pnpm.onlyBuiltDependencies`: only `electron` and `esbuild` can run postinstall scripts
- `preinstall: only-allow pnpm`: prevents accidental `npm install`
- `pnpm audit`: checks for known CVEs
- `pnpm-lock.yaml`: integrity hashes for every package

## File Conventions

- **Components**: PascalCase (`Avatar.tsx`)
- **Hooks**: camelCase with `use` prefix (`useBobeState.ts`)
- **Utilities**: camelCase (`constants.ts`)
- **Type files**: kebab-case (`bobe.ts`, `ipc.ts`)
- Import alias: `@/` maps to `src/`

## Important Documentation

- `docs/ARCHITECTURE_GUIDELINES.md` - Project structure, patterns, communication architecture, state management, loading states
- `docs/BEST_PRACTICES.md` - Security checklist, do/don't rules, performance, styling, testing
- `docs/ProjectDescription.md` - High-level project philosophy
- `docs/TODO.md` - Technical implementation notes and planned features
