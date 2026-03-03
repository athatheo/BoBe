# BoBe

**A local-first proactive AI companion for macOS that observes, remembers, and helps.**

BoBe lives on your desktop as a transparent overlay. It watches what you're working on, builds memories over time, tracks your goals, and reaches out when it thinks it can help — like a thoughtful colleague who actually pays attention.

Everything runs locally. Your data never leaves your machine unless you choose a cloud LLM provider.

---

## Features

| | |
|---|---|
| **Observes your work** | Periodic screen captures analyzed by a vision LLM to understand what you're doing |
| **Remembers you** | Short-term and long-term memory built from conversations and observations, powered by semantic embeddings |
| **Tracks your goals** | Extracts goals from conversations, persists them, and references them when relevant |
| **Reaches out proactively** | A decision engine evaluates when help would be valuable — not a chatbot waiting for input |
| **Respects your flow** | Cooldown logic and engagement awareness prevent interruptions when you're focused |
| **Uses tools** | Native file access, memory search, and extensible [MCP](https://modelcontextprotocol.io/) server integration |
| **Customizable personality** | Soul documents shape how BoBe communicates — make it formal, casual, technical, or anything in between |

---

## How It Works

BoBe is two processes that communicate over localhost:

```
bobe-daemon (Rust/Axum, :8766)  <── SSE + HTTP ──>  BoBe.app (Swift/SwiftUI)
         |                                                    |
         v                                                    v
   LLM Backend                                     Transparent overlay
   (Ollama / OpenAI / Azure / llama.cpp)           + settings + menu bar
```

The **Rust backend** handles all intelligence: screen capture, context assembly, LLM orchestration, learning pipelines, tool execution, and goal tracking. It exposes a REST + SSE API on `127.0.0.1:8766`.

The **Swift app** is a native macOS overlay — a floating avatar with chat bubbles, a message input, a settings panel, and a menu bar icon. It connects to the backend via SSE for real-time updates.

### LLM Providers

| Provider | Use Case |
|----------|----------|
| **[Ollama](https://ollama.ai)** | Recommended for local inference. Auto-started and managed by BoBe. |
| **OpenAI** | Cloud inference (gpt-5 family). Requires API key. |
| **Azure OpenAI** | Enterprise cloud inference. Requires endpoint + key + deployment. |
| **llama.cpp** | Direct local inference without Ollama. |

BoBe auto-detects your model's context window and clamps response budgets to prevent overflows.

---

## Quick Start

### Prerequisites

- **macOS 14+** (Sonoma)
- **[Ollama](https://ollama.ai)** (recommended) or an OpenAI/Azure API key
- **[just](https://github.com/casey/just)** task runner

For building from source:
- Rust 1.93+ (edition 2024)
- Xcode 16+ with Swift 6.0

### Install & Run

```bash
git clone https://github.com/user/bobe.git
cd bobe
just run
```

This builds the Rust backend and launches the macOS app. On first launch, a setup wizard guides you through choosing an LLM provider and pulling a model.

### Build a Release

```bash
just build          # Release build + bundle BoBe.app
just release 1.0.0  # Build + sign + create DMG
```

---

## Development Commands

### `just` recipes

| Command | What it does |
|---------|---------------|
| `just` | List all available recipes |
| `just run` | Build backend + frontend (debug) and launch the app |
| `just backend` / `just run-backend` | Run only the Rust daemon (`cargo run -- serve`) |
| `just build` | Build release binaries and bundle `build/BoBe.app` |
| `just release 1.0.0` | Build + sign app + create/sign DMG |
| `just ship <version> <apple-id> <team-id> <password>` | End-to-end ship flow (clean, resolve deps, release, notarize, staple, Sparkle zip) |
| `just clean` | Clean cargo, SwiftPM, and `build/` artifacts |
| `just check` / `just test` | Run Rust + Swift checks (`fmt`, clippy, tests, SwiftLint, Swift build) |
| `just format-swift` | Format Swift sources with SwiftFormat |
| `just check-swift-format` | Lint Swift formatting without rewriting files |
| `just xcode` | Regenerate Xcode project files via XcodeGen |
| `just sparkle-zip version=1.0.0` | Create Sparkle update zip from `build/BoBe.app` |
| `just sparkle-sign-update version=1.0.0` | Sign Sparkle zip archive with private Sparkle key |
| `just sparkle-generate-appcast ...` | Generate/update Sparkle `appcast.xml` |

### Backend (Rust daemon)

**Package manager / build tool:** `cargo`

**Code layout (`src/`)**
- `main.rs`: CLI entrypoint (`serve`, `version`) and server bootstrap
- `api/`: Axum routes + handlers
- `runtime/`: proactive session engine, triggers, learners, prompts
- `services/`: business logic and orchestration
- `db/`: SQLite repository layer (`sqlx`)
- `llm/`: model provider integrations (Ollama/OpenAI/Azure/llama.cpp)

**Common backend commands**

```bash
cargo fetch          # Download Rust dependencies
cargo build          # Debug build
cargo build --release
cargo run -- serve   # Run backend server on localhost:8766
cargo fmt --check    # Rust formatting check
cargo clippy -q      # Lints (pedantic profile configured in Cargo.toml)
cargo test -q        # Run backend tests
```

### Frontend (macOS SwiftUI app)

**Package manager / build tool:** Swift Package Manager (`swift package`)

**Code layout (`desktopMac/BoBe/`)**
- `App/`: app lifecycle, tray, overlay panel, updater
- `Views/`: overlay UI, settings screens, setup wizard
- `Stores/`: observable app state
- `Services/`: backend process + HTTP/SSE client
- `Models/`: API DTOs and domain view models

**Common frontend commands**

```bash
cd desktopMac
swift package resolve      # Resolve Swift dependencies
swift build -c debug       # Debug build
swift build -c release     # Release build
swiftlint lint --quiet     # Swift lint checks
swiftformat --lint BoBe    # Formatting check
swiftformat BoBe           # Apply formatting
```

---

## Configuration

All settings are configurable at runtime through the app's settings panel. They persist to `~/.bobe/config.toml`.

Environment variable overrides use the `BOBE_` prefix with `__` for nesting:

```bash
BOBE_LLM__BACKEND=openai          # Switch to OpenAI
BOBE_LLM__CONTEXT_WINDOW=8192     # Override auto-detected context window
BOBE_CAPTURE__ENABLED=false        # Disable screen capture
BOBE_CAPTURE__INTERVAL_SECONDS=30  # Capture every 30 seconds
```

Data is stored at `~/.bobe/` — SQLite database, config, goals file, and MCP server configuration.

---

## Domain Model

BoBe maintains several types of persistent knowledge:

| Concept | What It Is | Retention |
|---------|------------|-----------|
| **Soul** | Personality documents that shape LLM behavior | Permanent |
| **Goal** | Intentions extracted from conversations (active / completed / archived) | Until archived + 30 days |
| **Memory (short-term)** | Recent distilled facts from conversations and observations | 30 days |
| **Memory (long-term)** | Consolidated knowledge from the learning pipeline | 90 days |
| **Memory (explicit)** | Things you explicitly ask BoBe to remember | Permanent |
| **Observation** | Raw screen capture analysis data | 7 days |
| **Conversation** | Chat sessions with full turn history | Permanent |
| **User Profile** | Information about you that BoBe references in context | Permanent |

---

## API

The backend exposes a REST API on `127.0.0.1:8766`. Key endpoints:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/status` | GET | Runtime session state |
| `/events` | GET | SSE event stream (real-time updates) |
| `/message` | POST | Send a message to BoBe |
| `/goals` | GET/POST | List or create goals |
| `/goals/{id}` | GET/PATCH/DELETE | Goal CRUD + complete/archive |
| `/memories` | GET/POST | List or create memories |
| `/memories/search` | POST | Semantic memory search |
| `/souls` | GET/POST | Personality document management |
| `/user-profiles` | GET/POST | User profile management |
| `/settings` | GET/PATCH | Runtime configuration |
| `/models` | GET | List installed Ollama models |
| `/models/pull` | POST | Download a model (SSE progress) |
| `/tools` | GET | List available tools |
| `/tools/mcp/config` | GET/PUT/DELETE | MCP server configuration |
| `/onboarding/setup` | POST | Start setup wizard job |
| `/capture/start` | POST | Enable screen capture |
| `/capture/stop` | POST | Disable screen capture |

---

## Security

BoBe handles sensitive data (screen captures, API keys, personal context). Security is taken seriously:

- **Localhost only** — all endpoints bind to `127.0.0.1`, never exposed to the network
- **Host validation** middleware on every route
- **API keys** stored in macOS Keychain via the `secrecy` crate, never logged or written to disk in plaintext
- **File tools** use `canonicalize()` + ancestry checks to prevent path traversal
- **MCP commands** validated against a configurable blocklist
- **CORS** locked to localhost origins

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions, coding conventions, and architecture details.

## License

[MIT](LICENSE)
