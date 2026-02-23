# BoBe Shell - Best Practices

> Security checklist, do/don't rules, and configuration guide for Electron + React + Vite + Tailwind + Radix + Motion

**Related docs:**

- [ARCHITECTURE_GUIDELINES.md](./ARCHITECTURE_GUIDELINES.md) - Project structure, patterns, communication

---

## Table of Contents

1. [Security Checklist](#security-checklist)
2. [Electron Fuses](#electron-fuses)
3. [IPC Best Practices](#ipc-best-practices)
4. [React Best Practices](#react-best-practices)
5. [Performance Guidelines](#performance-guidelines)
6. [Styling Best Practices](#styling-best-practices)
7. [Animation Best Practices](#animation-best-practices)
8. [Radix UI Best Practices](#radix-ui-best-practices)
9. [Testing Guidelines](#testing-guidelines)
10. [Quick Reference: Do/Don't](#quick-reference-dodont)

---

## Security Checklist

### BrowserWindow Configuration

```typescript
new BrowserWindow({
  webPreferences: {
    // SECURITY CRITICAL - never change without understanding
    nodeIntegration: false, // ✅ REQUIRED
    contextIsolation: true, // ✅ REQUIRED
    sandbox: true, // ✅ REQUIRED
    webSecurity: true, // ✅ REQUIRED
    allowRunningInsecureContent: false, // ✅ REQUIRED
    preload: path.join(__dirname, 'preload.js'),
  },
})
```

### App-Level Security

```typescript
// Enable sandbox for all renderers
app.enableSandbox()

// Block navigation to unknown origins
win.webContents.on('will-navigate', (event, url) => {
  const allowed = ['http://localhost:5173', 'file://']
  if (!allowed.some((origin) => url.startsWith(origin))) {
    event.preventDefault()
  }
})

// Block new window creation
win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))

// CSP header
session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
  callback({
    responseHeaders: {
      ...details.responseHeaders,
      'Content-Security-Policy': ["default-src 'self'; script-src 'self'"],
    },
  })
})
```

### Preload Security Rules

| Rule                                     | Why                                        |
| ---------------------------------------- | ------------------------------------------ |
| Allowlist channels explicitly            | Prevents "call anything" backdoor          |
| Wrap callbacks to strip event object     | Event object exposes ipcRenderer internals |
| Never expose `ipcRenderer.send` directly | Allows arbitrary message sending           |
| Never expose `ipcRenderer.on` directly   | Allows listening to any channel            |
| Return structured errors                 | Don't leak stack traces to renderer        |

**Bad (insecure):**

```typescript
// NEVER DO THIS
contextBridge.exposeInMainWorld('api', {
  send: ipcRenderer.send,
  on: ipcRenderer.on,
})
```

**Good (secure):**

```typescript
// One method per action, validated channels
contextBridge.exposeInMainWorld('api', {
  startCapture: () => ipcRenderer.invoke('api:start-capture'),
  onStateChange: (cb) => {
    const handler = (_e, data) => cb(data)
    ipcRenderer.on('backend:state', handler)
    return () => ipcRenderer.removeListener('backend:state', handler)
  },
})
```

---

## Electron Fuses

Fuses are binary flags that can be flipped at package time for additional security.

### Recommended Fuse Configuration

```typescript
import { flipFuses, FuseVersion, FuseV1Options } from '@electron/fuses'

await flipFuses('/path/to/Your.app', {
  version: FuseVersion.V1,
  [FuseV1Options.RunAsNode]: false, // ✅ Disable
  [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false, // ✅ Disable
  [FuseV1Options.EnableNodeCliInspectArguments]: false, // ✅ Disable
  [FuseV1Options.EnableCookieEncryption]: true, // ✅ Enable
  [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true, // ✅ Enable
  [FuseV1Options.OnlyLoadAppFromAsar]: true, // ✅ Enable
  [FuseV1Options.GrantFileProtocolExtraPrivileges]: false, // ✅ Disable
})
```

| Fuse                                    | Default  | Recommendation | Why                                   |
| --------------------------------------- | -------- | -------------- | ------------------------------------- |
| `RunAsNode`                             | Enabled  | **Disable**    | Prevents ELECTRON_RUN_AS_NODE attacks |
| `EnableNodeOptionsEnvironmentVariable`  | Enabled  | **Disable**    | Blocks NODE_OPTIONS injection         |
| `EnableNodeCliInspectArguments`         | Enabled  | **Disable**    | Blocks --inspect in production        |
| `EnableCookieEncryption`                | Disabled | **Enable**     | Encrypts cookies with OS keys         |
| `EnableEmbeddedAsarIntegrityValidation` | Disabled | **Enable**     | Validates app.asar integrity          |
| `OnlyLoadAppFromAsar`                   | Disabled | **Enable**     | Prevents loading loose files          |
| `GrantFileProtocolExtraPrivileges`      | Enabled  | **Disable**    | Removes file:// special powers        |

---

## IPC Best Practices

### Do: Prefer invoke/handle

```typescript
// Main
ipcMain.handle('api:get-status', async () => {
  return await apiClient.getStatus()
})

// Renderer
const status = await window.bobe.getState()
```

### Do: Validate All Inputs in Main

```typescript
ipcMain.handle('fs:readFile', async (_event, filePath) => {
  // Type check
  if (typeof filePath !== 'string') {
    throw new Error('Invalid file path')
  }

  // Path validation
  const normalized = path.normalize(filePath)
  if (normalized.includes('..')) {
    throw new Error('Path traversal not allowed')
  }

  // Allowlist check
  const allowedDirs = [app.getPath('userData')]
  if (!allowedDirs.some((dir) => normalized.startsWith(dir))) {
    throw new Error('Access denied')
  }

  return fs.readFileSync(normalized, 'utf-8')
})
```

### Do: Name Channels Like APIs

**Good:**

- `bobe:get-state`
- `bobe:toggle-capture`
- `backend:state-update`

**Bad:**

- `read`
- `doThing`
- `message`

### Don't: Create Generic Send/Receive

```typescript
// NEVER DO THIS
ipcMain.handle('execute', (_, { channel, args }) => {
  return handlers[channel](...args) // RPC backdoor!
})
```

---

## React Best Practices

### Do: Keep State Local Until It Hurts

```tsx
// UI-only state stays in component
function ChatPanel() {
  const [isExpanded, setIsExpanded] = useState(false)
  // ...
}

// Shared state uses bobe-store hooks
const { state, actions } = useBobe()
```

### Do: Use Appropriate Hooks

```tsx
// Most components use combined hook
function Avatar() {
  const { state, toggleCapture } = useBobe()
}

// Performance-critical: subscribe to slice only
function StateDisplay() {
  const stateType = useBobeSelector((s) => s.stateType)
  return <div>{stateType}</div>
}

// Actions via bobeActions (no hook needed for fire-and-forget)
import { bobeActions } from '@/hooks'
function ActionButton() {
  return <button onClick={bobeActions.toggleCapture}>Toggle</button>
}
```

### Do: Use Stable References

```tsx
// Memoize callbacks that go to child components
const handleClick = useCallback(() => {
  toggleCapture()
}, [toggleCapture])

// Memoize computed values
const isActive = useMemo(() => state.capturing || state.thinking, [state.capturing, state.thinking])
```

### Don't: Derive State in Render

```tsx
// Bad - recalculates every render
function Component() {
  const { state } = useBobe()
  const derivedValue = expensiveCalculation(state) // ❌
}

// Good - use selector for derived state
function Component() {
  const derivedValue = useBobeSelector((s) => expensiveCalculation(s))
}
```

### Don't: Use Index as Key

```tsx
// Bad
{
  items.map((item, index) => <Item key={index} />)
}

// Good
{
  items.map((item) => <Item key={item.id} />)
}
```

---

## Performance Guidelines

### Do: Keep Renderers Light

- Each BrowserWindow is a full Chromium instance
- Use one main window unless you truly need more
- Lazy-load heavy UI sections

### Do: Watch Background Throttling

Electron throttles timers/animations when backgrounded. Use `backgroundThrottling: false` only if needed for your use case.

### Do: Profile Before Optimizing

```tsx
// React DevTools Profiler
<Profiler
  id="Avatar"
  onRender={(id, phase, actualDuration) => {
    console.log(`${id} ${phase}: ${actualDuration}ms`)
  }}
>
  <Avatar />
</Profiler>
```

### Don't: Update State Per Frame

```tsx
// Bad - tanks FPS
useEffect(() => {
  const id = requestAnimationFrame(() => {
    setState(newValue) // ❌ React render every frame
  })
  return () => cancelAnimationFrame(id)
})

// Good - use motion values
const x = useMotionValue(0)
useEffect(() => {
  const id = requestAnimationFrame(() => {
    x.set(newValue) // ✅ Bypasses React
  })
  return () => cancelAnimationFrame(id)
})
```

---

## Styling Best Practices

### Do: Use cn() for Class Composition

```tsx
import { cn } from '@/lib/cn'
;<div className={cn('base-class', isActive && 'active-class', className)} />
```

### Do: Use @theme for Design Tokens

```css
@theme {
  --color-bobe-terracotta: #c67b5c;
  --avatar-outer: 116px;
}

.avatar-card {
  width: var(--avatar-outer);
  background: var(--color-bobe-terracotta);
}
```

### Don't: Mix Styling Approaches

```tsx
// Bad - inconsistent
<div
  className="p-4"
  style={{ color: 'red', fontSize: '14px' }}
/>

// Good - one approach
<div className="p-4 text-red-500 text-sm" />
```

### Don't: Add Sass/Less on Top of Tailwind 4

Tailwind 4 is designed to be the preprocessor. Adding another one creates complexity.

---

## Animation Best Practices

### Do: Animate Transform and Opacity

```tsx
// Good - GPU accelerated
<motion.div
  animate={{ scale: 1.1, opacity: 0.8 }}
/>

// Avoid - triggers layout
<motion.div
  animate={{ width: '200px', height: '200px' }}
/>
```

### Do: Use AnimatePresence for Exit Animations

```tsx
<AnimatePresence mode="wait">
  {isVisible && (
    <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} />
  )}
</AnimatePresence>
```

### Do: Respect Reduced Motion

```tsx
const prefersReducedMotion = useReducedMotion()

<motion.div
  animate={{ scale: prefersReducedMotion ? 1 : 1.1 }}
/>
```

### Don't: Animate Everything

**Good uses:**

- State change feedback
- Navigation transitions
- Attention guidance

**Bad uses:**

- Every hover effect wiggles
- Constant background movement
- Pointless infinite loops

---

## Radix UI Best Practices

### Do: Use asChild Pattern

```tsx
<Dialog.Trigger asChild>
  <button className="btn">Open</button>
</Dialog.Trigger>
```

### Do: Keep Focus Management

Never remove focus outlines. Style them appropriately:

```css
.focus-ring {
  @apply outline-none ring-2 ring-bobe-terracotta ring-offset-2;
}
```

### Don't: Mix Headless UI Libraries

Don't combine Radix with Headless UI or other focus management libraries. They conflict.

---

## Testing Guidelines

### Do: Test Renderer Like a Web App

```tsx
// Unit test components
import { render, screen } from '@testing-library/react'

test('Avatar shows state', () => {
  render(<Avatar stateType="capturing" />)
  expect(screen.getByText('ON')).toBeInTheDocument()
})
```

### Do: Test IPC as a Contract

```typescript
test('get-state returns BobeState', async () => {
  const result = await ipcMain.handle('bobe:get-state')
  expect(result).toHaveProperty('capturing')
  expect(result).toHaveProperty('stateType')
})

test('rejects invalid input', async () => {
  await expect(ipcMain.handle('fs:readFile', '../../../etc/passwd')).rejects.toThrow()
})
```

### Do: Smoke Test Packaged Builds

Many bugs only appear after bundling/signing. Test the actual .app/.exe.

---

## Voice & Microphone (macOS)

### The Problem

On macOS, `getUserMedia()` can return a `MediaStream` with a "live" audio track that produces **silent/empty data**. This happens in two scenarios:

1. **Fresh permission grant**: `systemPreferences.askForMediaAccess('microphone')` triggers the macOS prompt. Even after the user clicks "Allow", the Chromium audio pipeline in the renderer may not immediately pick up the change.

2. **Bluetooth devices (AirPods)**: macOS switches AirPods to a low-quality SCO codec for microphone input. The sample rate mismatch (48kHz reported vs 24kHz actual) causes `MediaRecorder` to produce blobs with `size === 0` even though the track reports `enabled=true, muted=false, readyState=live`.

### The Fix: No Timeslice + Direct MediaRecorder

The `VoiceRecorder` (`src/lib/voice-recorder.ts`) uses `MediaRecorder` without a timeslice parameter. Calling `start()` with no argument collects the full recording into a single blob on `stop()`, which avoids the truncated-chunk issue with Bluetooth codec switching.

```typescript
// No timeslice — collect full blob on stop
this.mediaRecorder = new MediaRecorder(stream)
this.mediaRecorder.start() // NOT start(1000)
```

**Why not AudioContext routing**: `AudioContext` on macOS can error with `"The AudioContext encountered an error from the audio device"` when AirPods switch between codecs. Direct `MediaRecorder` without timeslice is more reliable.

**Known limitation**: After macOS permission resets (`tccutil reset`) or OS-level audio disruptions, `MediaRecorder` may produce empty webm containers (valid header, no audio payload). A reboot resolves this. **Never use `tccutil reset` as a debugging tool** — it corrupts the audio subsystem state.

### Permission Flow

```
App startup (main process)
  │
  ├─ systemPreferences.getMediaAccessStatus('microphone')
  │   └─ 'granted' → continue
  │   └─ 'not-determined' → askForMediaAccess('microphone') → macOS prompt
  │
  ├─ security.ts: setPermissionCheckHandler (allow media.audio)
  ├─ security.ts: setPermissionRequestHandler (allow media types=['audio'])
  │
  └─ createOverlayWindow()  ← renderer starts AFTER permission granted
```

### Dev Mode: Resetting Permissions

During development, macOS can invalidate microphone permissions when the Electron binary changes (rebuilds). To fix:

1. Open **System Settings > Privacy & Security > Microphone**
2. Toggle the Electron entry off then on
3. Restart the app

**Do NOT use `tccutil reset`** — it corrupts the CoreAudio pipeline and requires a reboot to recover.

### Key Files

| File                            | Role                                              |
| ------------------------------- | ------------------------------------------------- |
| `src/lib/voice-recorder.ts`     | MediaRecorder capture, error handling             |
| `electron/services/security.ts` | Permission check/request handlers (lines 174-199) |
| `electron/main/index.ts`        | `askForMediaAccess` on startup (lines 82-89)      |

---

## Quick Reference: Do/Don't

### Security

| Do                        | Don't                      |
| ------------------------- | -------------------------- |
| Enable `contextIsolation` | Disable `webSecurity`      |
| Enable `sandbox`          | Enable `nodeIntegration`   |
| Allowlist IPC channels    | Expose raw `ipcRenderer`   |
| Validate inputs in main   | Trust renderer input       |
| Use CSP headers           | Allow arbitrary navigation |

### React

| Do                         | Don't                             |
| -------------------------- | --------------------------------- |
| Use `useSyncExternalStore` | Use `useState` for external state |
| Split state/action hooks   | Combine everything in one hook    |
| Memoize callbacks          | Create functions in render        |
| Use stable keys            | Use array index as key            |

### Performance

| Do                              | Don't                        |
| ------------------------------- | ---------------------------- |
| Lazy load heavy components      | Load everything upfront      |
| Use motion values for animation | Update React state per frame |
| Profile before optimizing       | Premature optimization       |
| Keep devtools off in production | Ship with devtools enabled   |

### Styling

| Do                               | Don't                             |
| -------------------------------- | --------------------------------- |
| Use `cn()` for class composition | Use template literals for classes |
| Use CSS variables for tokens     | Hard-code values everywhere       |
| Use Tailwind utilities           | Mix multiple styling approaches   |
| Use `@theme` directive           | Add Sass/Less preprocessors       |

### Animation

| Do                               | Don't                             |
| -------------------------------- | --------------------------------- |
| Animate transform/opacity        | Animate width/height/layout       |
| Use `AnimatePresence`            | Mount/unmount without transitions |
| Respect `prefers-reduced-motion` | Ignore accessibility preferences  |
| Keep animations meaningful       | Animate everything                |

### IPC

| Do                       | Don't                              |
| ------------------------ | ---------------------------------- |
| Use `invoke/handle`      | Use `send/on` for request/response |
| Name channels like APIs  | Use generic names                  |
| Validate in main process | Trust renderer data                |
| Return structured errors | Expose stack traces                |

---

## Configuration Checklist

### Electron Window

- [ ] `nodeIntegration: false`
- [ ] `contextIsolation: true`
- [ ] `sandbox: true`
- [ ] `webSecurity: true`
- [ ] Preload with minimal API
- [ ] Navigation allowlist
- [ ] CSP header

### Fuses (Package Time)

- [ ] `RunAsNode: false`
- [ ] `EnableNodeOptionsEnvironmentVariable: false`
- [ ] `EnableNodeCliInspectArguments: false`
- [ ] `EnableCookieEncryption: true`
- [ ] `EnableEmbeddedAsarIntegrityValidation: true`
- [ ] `OnlyLoadAppFromAsar: true`

### Build

- [ ] Vite `base: "./"` for file:// compatibility
- [ ] Source maps disabled in production
- [ ] Devtools disabled in production
- [ ] Code signing configured
- [ ] Auto-update planned

---

## References

- [Electron Security](https://electronjs.org/docs/latest/tutorial/security)
- [Electron Context Isolation](https://electronjs.org/docs/latest/tutorial/context-isolation)
- [Electron Sandbox](https://electronjs.org/docs/latest/tutorial/sandbox)
- [React 19](https://react.dev/blog/2024/12/05/react-19)
- [Tailwind v4](https://tailwindcss.com/docs)
- [Radix Primitives](https://www.radix-ui.com/primitives)
- [Motion Performance](https://motion.dev/docs/performance)
