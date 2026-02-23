# BoBe Shell

Electron + React desktop overlay for the BoBe proactive AI companion. This is a **thin client** that communicates with the Python backend daemon via HTTP/SSE.

## Prerequisites

- Node.js 20+
- The [BoBe backend service](../ProactiveAI) running locally

## Quick Start

### 1. Start the Backend Service

In the backend repo (`ProactiveAI`):

```bash
cd /Users/john/Repos/ProactiveAI
uv run bobe serve
```

With logging to file:

```bash
BOBE_LOG_FILE="~/.bobe/logs/bobe.log" uv run bobe serve
```

To tail the backend logs:

```bash
tail -f ~/.bobe/logs/bobe.log
```

The backend runs on `http://localhost:8766` by default.

### 2. Start the Frontend (This Repo)

```bash
npm install
npm run dev
```

This starts the Electron app with hot reload. The overlay window will appear in the bottom-right corner.

## Development Commands

```bash
npm run dev          # Start Electron with hot reload (Vite dev server)
npm run build        # Production build (outputs to out/)
npm run preview      # Preview production build
npm run typecheck    # Run both node and app type checks
```

## Testing the App

1. **Connection**: The avatar shows a loading state initially. Once connected to the backend, it transitions to idle with a green connection dot.

2. **Send a message**: Click the chat icon (top-left of avatar) to open the message input. Type and press Enter to send.

3. **View response**: The AI response streams in and the speech bubble auto-appears with the complete message.

4. **Toggle capture**: Click the ON/OFF button below the avatar to toggle screen capture (requires backend support).

5. **Dismiss message**: Click the X on the speech bubble to dismiss it.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Electron App                              │
├─────────────────────────────────────────────────────────────┤
│  Main Process          │  Preload          │  Renderer      │
│  ─────────────         │  ───────          │  ────────      │
│  - DaemonClient        │  - contextBridge  │  - React UI    │
│  - IPC handlers        │  - window.bobe    │  - State store │
│  - Window manager      │                   │  - Components  │
│  - Tray manager        │                   │                │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ HTTP/SSE (localhost:8766)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Python Backend                            │
│  - Litestar API                                             │
│  - SSE event stream (/events)                               │
│  - LLM integration (Ollama/OpenAI)                          │
│  - Screen capture & context learning                        │
└─────────────────────────────────────────────────────────────┘
```

### Communication Flow

1. **Commands** (user actions): Renderer → IPC → Main Process → HTTP POST → Backend
2. **Events** (state updates): Backend → SSE → Main Process → IPC → Renderer

### SSE Events

The backend pushes events via Server-Sent Events:

| Event Type   | Description                                                       |
| ------------ | ----------------------------------------------------------------- |
| `indicator`  | UI state change (idle, capturing, thinking, generating, speaking) |
| `text_delta` | Streaming text chunk with `delta`, `sequence`, `done`             |
| `heartbeat`  | Keep-alive signal                                                 |
| `error`      | Error notification                                                |

### State Mapping

| Backend Indicator | Frontend State                                |
| ----------------- | --------------------------------------------- |
| `idle`            | idle                                          |
| `capturing`       | capturing                                     |
| `analyzing`       | thinking                                      |
| `thinking`        | thinking                                      |
| `generating`      | thinking → wants_to_speak (when text arrives) |
| `speaking`        | speaking                                      |

## Project Structure

```
├── electron/
│   ├── main/           # Main process entry
│   ├── preload/        # Context bridge (window.bobe API)
│   ├── ipc/            # IPC handlers, state management
│   ├── services/       # DaemonClient, WindowManager, TrayManager
│   └── types/          # TypeScript types (daemon.ts, index.ts)
│
├── src/
│   ├── app/            # App root, providers
│   ├── components/     # Avatar, SpeechBubble, MessageInput, indicators
│   ├── features/       # OverlayWindow feature
│   ├── hooks/          # bobe-store (useSyncExternalStore)
│   ├── types/          # bobe.ts, ipc.ts
│   └── styles/         # globals.css (Tailwind + design tokens)
```

## Troubleshooting

### App shows "loading" state

- Check that the backend is running: `curl http://localhost:8766/health`
- Check backend logs for errors
- The frontend auto-reconnects with exponential backoff

### Messages not appearing

- Check the Electron console (DevTools) for SSE events
- Verify backend is sending `text_delta` events with `done: true`
- Check that `lastMessage` is being set in state broadcasts

### Connection keeps dropping

- Check backend stability
- Look for `ECONNRESET` errors in frontend logs
- Backend may be restarting or hitting timeouts

## Backend API Reference

Get the full API schema from the backend:

```bash
cd /Users/john/Repos/ProactiveAI
/Users/john/Repos/ProactiveAI/scripts/dump_api_schema.sh
```

Key endpoints:

- `GET /health` - Health check
- `GET /status` - Current state
- `GET /events` - SSE stream
- `POST /message` - Send user message
- `POST /capture/start` - Start capture
- `POST /capture/stop` - Stop capture
