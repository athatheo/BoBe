# Contributing to BoBe

Thanks for your interest in contributing! BoBe is an open-source project and we welcome contributions of all kinds.

## Getting Started

### Prerequisites

- **macOS 14+** (Sonoma)
- **Rust 1.93+** (edition 2024)
- **Xcode 16+** with Swift 6.0
- **[just](https://github.com/casey/just)** task runner
- **[Ollama](https://ollama.ai)** for local LLM inference (or an OpenAI/Azure API key)
- **SwiftLint** for Swift linting

### Setup

```bash
git clone https://github.com/athatheo/BoBe.git
cd Bobe

# Check everything builds and passes
just check
```

`just check` runs: `cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo deny check`, `cargo machete`, `swiftlint`, and `swift build`.

`just check-ci` is the stricter CI-facing variant: it uses `--locked` for Cargo resolution.

## Architecture

BoBe is two processes that communicate over localhost:

```
bobe-daemon (Rust/Axum, :8766)  ←── SSE + HTTP ──→  BoBe.app (Swift/SwiftUI)
         │                                                   │
         ▼                                                   ▼
   LLM Backend                                    Transparent overlay
   (Ollama/OpenAI/Azure/llama.cpp)                + settings + tray
```

The **Rust backend** handles all intelligence: screen capture, context assembly, LLM orchestration, learning pipelines, tool execution, and goal tracking. It exposes a REST + SSE API on `127.0.0.1:8766`.

The **Swift app** is a native macOS overlay — a floating avatar with chat bubbles, a message input, a settings panel, and a menu bar icon.

### Architecture Principles

- **Constructor injection** via `AppState` (Arc-wrapped, Axum State extractor) — no DI framework
- **Layered architecture**: Handler → Service → Repository (some simple handlers call repos directly)
- **Trait-based abstraction** for LLM providers, embedding, and repositories
- **Hot-swappable config** via `ArcSwap` — settings changes apply without restart
- **Localhost-only** by design — all network traffic stays on `127.0.0.1`

## Development Commands

### `just` Recipes

| Command | What it does |
|---------|---------------|
| `just` | List all available recipes |
| `just run` | Build backend + frontend (debug) and launch the app |
| `just backend` / `just run-backend` | Run only the Rust daemon (`cargo run -- serve`) |
| `just build` | Build release binaries and bundle `build/BoBe.app` |
| `just release 1.0.0` | Build + sign app + create/sign DMG |
| `just ship <ver> <apple-id> <team> <pw>` | End-to-end ship flow (clean → build → sign → DMG → notarize → staple → Sparkle) |
| `just clean` | Clean cargo, SwiftPM, and `build/` artifacts |
| `just check` / `just test` | Run Rust + Swift checks (fmt, clippy, tests, SwiftLint, Swift build) |
| `just format-swift` | Format Swift sources with SwiftFormat |
| `just check-swift-format` | Lint Swift formatting without rewriting files |
| `just xcode` | Regenerate Xcode project files via XcodeGen |
| `just sparkle-zip version=1.0.0` | Create Sparkle update zip from `build/BoBe.app` |
| `just sparkle-sign-update version=1.0.0` | Sign Sparkle zip archive with private Sparkle key |
| `just sparkle-generate-appcast ...` | Generate/update Sparkle `appcast.xml` |

### Backend Commands (Rust)

```bash
cd BoBeService
cargo fetch          # Download Rust dependencies
cargo build          # Debug build
cargo build --release
cargo run -- serve   # Run backend server on localhost:8766
cargo fmt --check    # Rust formatting check
cargo clippy -q      # Lints (pedantic profile configured in Cargo.toml)
cargo test -q        # Run backend tests
```

### Frontend Commands (Swift)

```bash
cd BoBeMacUI
swift package resolve      # Resolve Swift dependencies
swift build -c debug       # Debug build
swift build -c release     # Release build
swiftlint lint --quiet     # Swift lint checks
swiftformat --lint BoBe    # Formatting check
swiftformat BoBe           # Apply formatting
```

## Project Structure

```
BoBeService/                  # Rust backend (bobe-daemon)
  Cargo.toml                  # Rust dependencies and build config
  src/
    main.rs                   # CLI entrypoint (serve, version)
    api/                      # Axum routes and handlers
    app_state.rs              # Arc-wrapped DI container
    binary_manager/           # Ollama binary download/extraction
    bootstrap/                # Dependency wiring and startup
    config.rs                 # Configuration (BOBE_* env vars)
    config_manager/           # Runtime hot-swap config
    db/                       # SQLite repositories (sqlx)
    i18n/                     # Internationalization (Fluent)
    llm/                      # LLM provider abstraction
    models/                   # Domain structs
    runtime/                  # Session state, learners, triggers, prompts
    secrets.rs                # macOS Keychain integration
    services/                 # Business logic layer
    tools/                    # Native tools + MCP integration
    util/                     # SSE, capture, tokens, text utils
  migrations/                 # SQLite schema (auto-run on startup)
  deny.toml                   # cargo-deny license/ban policy

BoBeMacUI/                    # Swift macOS app (BoBe.app)
  BoBe/App/                   # App delegate, overlay panel, tray
  BoBe/Features/Settings/     # Settings panels (AI model, behavior, etc.)
  BoBe/Models/                # API DTOs, entity types
  BoBe/Services/              # Backend lifecycle, HTTP + SSE client
  BoBe/Stores/                # Observable state stores
  BoBe/Theme/                 # Theme configuration
  BoBe/Views/                 # Overlay UI + setup wizard

docs/                         # Additional documentation
```

## Coding Conventions

### Rust

- **Edition 2024**, MSRV 1.93, `unsafe_code = "deny"`
- **Clippy pedantic** enabled with justified allows (see `Cargo.toml`)
- Errors via `thiserror`, handlers return `Result<T, AppError>` — no `unwrap()`/`expect()` outside tests
- LLM prompt templates live in `runtime/prompts/` (some supplementary prompts in `tools/preselector.rs` and `i18n/`)
- Configuration via `BOBE_*` env vars, persisted to `~/.bobe/config.toml`
- API keys stored in macOS Keychain via `security-framework`, handled in-memory with the `secrecy` crate
- Follow [docs/RUST_GUIDELINES.md](docs/RUST_GUIDELINES.md) for architecture and style

### Swift

- Swift 6.0, macOS 14+ target
- **SwiftLint** enforced (see `BoBeMacUI/.swiftlint.yml`)
- `sorted_imports` required, `force_unwrapping` discouraged
- Split large views into focused subviews

### General

- Keep functions under 50 lines, files under 500 lines
- No global package installs — all dependencies stay in the project
- Commit messages: `type: short description` (e.g., `feat:`, `fix:`, `chore:`)

## Domain Model

BoBe maintains several types of persistent knowledge:

| Concept | What It Is | Retention |
|---------|------------|-----------|
| **Soul** | Personality documents that shape LLM behavior | Permanent |
| **Goal** | Intentions extracted from conversations | Until archived + 30 days |
| **Memory (short-term)** | Distilled facts from conversations and observations | 30 days |
| **Memory (long-term)** | Consolidated knowledge from the learning pipeline | 90 days |
| **Memory (explicit)** | Things you explicitly ask BoBe to remember | Permanent |
| **Observation** | Raw screen capture analysis data | 7 days |
| **Conversation** | Chat sessions with full turn history | Permanent |
| **User Profile** | Information about you that BoBe references in context | Permanent |

## API Reference

The backend exposes a REST API on `127.0.0.1:8766`:

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
| `/onboarding/status` | GET | Onboarding completion status |
| `/onboarding/setup` | POST | Start setup wizard job |
| `/onboarding/setup/{job_id}` | GET/DELETE | Check or cancel a setup job |
| `/goal-plans` | GET | List goal worker plans |
| `/goal-plans/status` | GET | Goal worker status |
| `/goal-plans/{plan_id}` | GET | Get a specific goal plan |
| `/goal-plans/{plan_id}/approve` | POST | Approve a goal plan |
| `/capture/start` | POST | Enable screen capture |
| `/capture/stop` | POST | Disable screen capture |

## Development Workflow

1. **Fork and branch** from `main`
2. **Make your changes** following the coding conventions above
3. **Run `just check`** to verify everything passes
4. **Submit a pull request** with a clear description

### Dependency Review

Dependency-changing pull requests get extra scrutiny. Call out any new or materially changed:

- proc-macro crates
- `build.rs` crates
- `-sys` / FFI crates
- git dependencies or new registries
- crates with broad network, filesystem, archive, parser, or subprocess reach

For supply-chain-sensitive changes, CI enforces `cargo deny` and deterministic Cargo resolution.

### CI and Release Model

BoBe uses two lanes:

- **Public vetting CI** — PR/push validation with no release secrets
- **Protected release workflow** — macOS signing, notarization, Sparkle signing, and update publishing

Release secrets must never be used in normal CI. Workflow files, release scripts, OTA docs, and entitlements should be reviewed carefully because they sit on the release control plane.

Maintainers should read [docs/UpdatingOTA.md](docs/UpdatingOTA.md) before changing CI, release scripts, signing, notarization, or Sparkle publishing behavior.

## Security

BoBe handles screen captures and LLM API keys. Please be mindful of:

- All endpoints bind to `127.0.0.1` only
- Host validation middleware on all routes
- File tool access uses `canonicalize()` + ancestry checks
- MCP commands are validated against a blocklist
- API keys go through macOS Keychain, never logged or persisted in plaintext
- Release signing, notarization, Sparkle, and update-host credentials belong only in protected CI environments

See [SECURITY.md](SECURITY.md) for our vulnerability reporting policy.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
