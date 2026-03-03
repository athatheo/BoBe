# BoBe (Rust)

**Local-first proactive AI companion that observes, remembers, and helps — rewritten in Rust.**

This is a complete Rust port of the [BoBe Python service](https://github.com/user/BoBe). Same architecture, same API contract, same features — but compiled, fast, and with a single binary.

---

## What BoBe Does

BoBe inverts the typical AI interaction. Instead of you loading context into the AI, the AI already knows:

| Capability | How It Works |
|------------|--------------|
| **Observes your work** | Screenshots every N seconds, OCR, vision LLM analysis |
| **Remembers short-term** | Recent activities stored with semantic embeddings |
| **Remembers long-term** | Background learning distills observations into lasting memories |
| **Tracks your goals** | Extracts goals from conversations, persists and references them |
| **Reaches out proactively** | Decides when help would be valuable — like a thoughtful colleague |
| **Respects your flow** | Cooldowns prevent spam; learns when to stay quiet |
| **Executes tools** | Native tools + MCP server integration for file access, memory search, coding agents |

---

## Architecture

Three processes work together:

| Process | Technology | Responsibility |
|---------|------------|----------------|
| **bobe** | Rust / Axum | All business logic: orchestration, capture, learning, tools |
| **bobe-shell** | Swift / SwiftUI (macOS) | Native desktop overlay that displays state via SSE |
| **LLM backend** | Ollama / llama.cpp / OpenAI / Azure OpenAI | Local or cloud inference |

```bash
┌─────────────────────────────────────────────────────────────┐
│                      USER'S DESKTOP                          │
│                                                              │
│  Screen Activity ──┐                                         │
│                    ▼                                         │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                   bobe (Rust/Axum)                     │  │
│  │                                                        │  │
│  │  Capture ──▶ Analyze ──▶ Remember ──▶ Decide ──▶ Help  │  │
│  └───────────────────────────────────────────────────────┘  │
│                    │ SSE + HTTP                               │
│                    ▼                                         │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              bobe-shell (Swift/SwiftUI)                 │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Internal Structure

```bash
src/
├── main.rs                  # Entry point + CLI (clap)
├── config.rs                # Settings from BOBE_* env vars
├── error.rs                 # Shared error types
├── app_state.rs             # Arc-wrapped shared singletons
├── domain/                  # Pure structs + enums (no framework deps)
├── ports/                   # Trait definitions (interfaces)
├── application/             # Business logic (learners, triggers, runtime, services, prompts)
├── adapters/                # Implementations (LLM providers, DB repos, tools, SSE, capture)
├── entrypoints/             # HTTP handlers (Axum controllers)
├── composition/             # Wiring, bootstrap, config management
└── shared/                  # Leaf utilities (IDs, clock)
```

---

## Quick Start

### Prerequisites

- Rust 1.75+ (edition 2021)
- SQLite 3.35+ (bundled via sqlx)
- [Ollama](https://ollama.ai) (recommended) or llama.cpp or OpenAI API key

### Build & Run

```bash
cargo build --release
./target/release/bobe serve
```

Or during development:

```bash
cargo run -- serve --host 127.0.0.1 --port 8765
```

Server starts at <http://127.0.0.1:8765>

- Health check: <http://127.0.0.1:8765/health>
- SSE stream: <http://127.0.0.1:8765/events>

## Domain Model

| Concept | Description | Retention |
|---------|-------------|-----------|
| **Soul** | Personality documents injected into LLM prompts | Forever |
| **Goal** | User intentions (active → completed/archived) | Until archived + 30d |
| **Memory (short)** | Recent distilled facts | 30 days |
| **Memory (long)** | Consolidated knowledge | 90 days |
| **Memory (explicit)** | User-requested "remember this" | Forever |
| **Observation** | Raw screen capture data | 7 days |
| **Conversation** | Chat session (PENDING → ACTIVE → CLOSED) | Forever |
| **Cooldown** | Proactive engagement timestamps (single row) | N/A |

---

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/status` | GET | Runtime session status |
| `/events` | GET | SSE event stream |
| `/conversation/message` | POST | Send a message |
| `/conversation/history` | GET | Conversation history |
| `/conversation/close` | POST | Close conversation |
| `/capture` | POST | Trigger manual capture |
| `/capture/start` | POST | Enable capture cycle |
| `/capture/stop` | POST | Disable capture cycle |
| `/context/search` | POST | Semantic search |
| `/context/recent` | GET | Recent context items |
| `/goals` | GET/POST | List/create goals |
| `/goals/{id}` | GET/PUT/DELETE | Goal CRUD |
| `/memories` | GET | List memories |
| `/memories/{id}` | PUT/DELETE | Memory CRUD |
| `/souls` | GET/POST | List/create souls |
| `/souls/{id}` | GET/PUT/DELETE | Soul CRUD |
| `/tools` | GET | List available tools |
| `/tools/mcp/config` | GET/PUT/DELETE | MCP full-document config (read/save/reset) |
| `/tools/mcp/config/validate` | POST | MCP config validation + tool discovery preview |
| `/settings` | GET/PATCH | Runtime settings |
| `/models` | GET | List installed models |
| `/models/registry` | GET | Browse model registry |
| `/models/pull` | POST | Download model (SSE progress) |
| `/models/{name}` | DELETE | Delete model |
| `/onboarding/status` | GET | Setup status |
| `/onboarding/options` | GET | Setup options (tiers/providers/models) |
| `/onboarding/setup` | POST | Start idempotent setup job |
| `/onboarding/setup/{job_id}` | GET/DELETE | Poll or cancel setup job |
| `/onboarding/mark-complete` | POST | Mark setup done |
| `/user-profile` | GET/PUT | User profile |

---

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- serve

# Check without building
cargo check

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

### Database Migrations

```bash
# Install sqlx-cli
cargo install sqlx-cli --features sqlite

# Run migrations
sqlx migrate run --source migrations/

# Create new migration
sqlx migrate add <name> --source migrations/
```

---

## License

MIT
