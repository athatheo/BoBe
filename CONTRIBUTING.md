# Contributing to BoBe

Thanks for your interest in contributing! BoBe is an open-source project and we welcome contributions of all kinds.

## Getting Started

### Prerequisites

- **macOS 15+** (Sequoia)
- **Rust 1.97.1+** (edition 2024)
- **Xcode 16+** with Swift 6.0
- **[just](https://github.com/casey/just)** task runner
- A GitHub Copilot subscription for cloud inference, or [Ollama](https://ollama.ai) for local inference

### Setup

```bash
git clone https://github.com/athatheo/BoBe.git
cd BoBe

# Install pinned cargo-deny, cargo-machete, SwiftLint, and XcodeGen binaries
# under this checkout's ignored .tools/ directory.
just bootstrap-tools

# Check everything builds and passes
just check
```

`just check` runs Rust formatting, Clippy, Rust and Swift tests, dependency audits, cross-language and localization catalog checks, SwiftLint, a Swift debug build, and runtime-artifact validation.

## Architecture

BoBe is two processes that communicate over localhost:

```
bobe-daemon (Rust/Axum, :8766)  ←── SSE + HTTP ──→  BoBe.app (Swift/SwiftUI)
         │                                                   │
         ▼                                                   ▼
   Copilot SDK or Ollama                          Transparent overlay
                                                    + settings + menu bar
```

The **Rust daemon** owns conversations, memory, goals, policy, Copilot sessions, MCP, proactive triggers, persistence, server TTS, and the REST/SSE/WebSocket API. It binds to `127.0.0.1:8766` by default and has an explicit authenticated HTTPS remote mode.

The **Swift app** owns the native macOS overlay, setup/settings windows, menu-bar integration, microphone/STT, playback, permissions, and local daemon supervision.

### Architecture Principles

- **Constructor injection** via `AppState` (Arc-wrapped, Axum State extractor) — no DI framework
- **Layered architecture**: handlers extract/respond; services and runtime own behavior; repositories own SQLite access
- **Concrete types by default**, traits only at real substitution or external-I/O seams
- **Hot-swappable config** via `ArcSwap` — settings changes apply without restart
- **One personal authority** — Swift presents and captures; the daemon owns durable intelligence and policy
- **Loopback-safe default**, with remote access requiring allowed hosts, bearer authentication, and TLS

## Development Commands

### `just` Recipes

| Command | What it does |
|---------|---------------|
| `just` | List all available recipes |
| `just run` | Build backend + frontend (debug) and launch the app |
| `just backend` / `just run-backend` | Stage the pinned Copilot helper and run only the Rust daemon |
| `just build` | Build release binaries and bundle `build/BoBe.app`, including the signed Copilot helper |
| `just profile-startup` | Measure daemon health readiness, Copilot prewarm, RSS, and binary size |
| `just smoke-copilot-tools` | Opt-in real SDK smoke for memory/goal tools and privacy purge |
| `just smoke-bodylink` | Opt-in mTLS BodyLink simulator through the real daemon/agent/TTS path |
| `just smoke-bodylink-swift` | Same BodyLink flow through the real Swift FluidAudio/Opus adapter |
| `just check-bodylink-contract` | Verify Rust, Swift, and ESP BodyLink constants agree |
| `just bootstrap-tools` | Install the pinned developer CLI toolchain under `.tools/` |
| `just release 1.0.0` | Build + sign app + create/sign DMG |
| `just ship 1.0.0 "Developer ID Application"` | End-to-end ship flow (clean → build → sign → DMG → notarize → staple → Sparkle) |
| `just clean` | Clean cargo, SwiftPM, and `build/` artifacts |
| `just check` / `just test` | Run Rust + Swift checks, tests, audits, contract drift checks, and builds |
| `just xcode` | Regenerate Xcode project files via XcodeGen |
| `just sparkle-zip 1.0.0` | Create Sparkle update zip from `build/BoBe.app` |
| `just sparkle-sign-update 1.0.0 /path/to/sparkle-private-key` | Sign Sparkle zip archive with private Sparkle key |
| `just sparkle-generate-appcast build/sparkle https://example.com/updates /path/to/sparkle-private-key` | Generate/update Sparkle `appcast.xml` |

Runtime recipes build the daemon without embedding the Copilot CLI archive.
The SDK downloads and SHA-verifies its pinned asset, stages the executable under
`build/copilot-cli/runtime`, and app assembly installs it at
`BoBe.app/Contents/Helpers/copilot`. Static checks skip the CLI download.
Direct Cargo builds retain the self-contained embedded fallback feature.

The Copilot smoke uses `gpt-5.6-sol` by default and consumes AI credits. Override it with `BOBE_SMOKE_MODEL`; use `BOBE_SMOKE_PORT` or `BOBE_PROFILE_PORT` when the default local probe ports are occupied.

### Backend Commands (Rust)

```bash
cd BoBeService
cargo fetch --locked          # Download Rust dependencies
cargo build --locked          # Debug build
cargo build --release --locked
cargo run --locked -- serve   # Run backend server on localhost:8766
cargo fmt --check                  # Rust formatting check
cargo clippy -q --locked           # Lints (pedantic profile configured in Cargo.toml)
cargo test -q --locked             # Run backend tests
../.tools/bin/cargo-deny check     # Dependency policy
../.tools/bin/cargo-machete        # Unused direct dependencies
```

### Frontend Commands (Swift)

```bash
cd BoBeMacUI
swift package resolve      # Resolve Swift dependencies
swift build -c debug       # Debug build
swift build -c release     # Release build
swift test                 # Swift unit tests
../.tools/bin/swiftlint lint --quiet     # Swift lint checks
```

## Project Structure

```
BoBeService/                  # Rust backend (bobe-daemon)
  Cargo.toml                  # Rust dependencies and build config
  src/
    main.rs                   # CLI entrypoint (serve, version)
    api/                      # Axum routes and handlers
    app_state.rs              # Arc-wrapped DI container
    bootstrap/                # Dependency wiring and startup
    config/                   # BOBE_* config, persistence, runtime hot-swap
    copilot/                  # SDK client, workers, hooks, sessions, skills, domain tools
    db/                       # SQLite repositories (sqlx)
    mcp/                      # MCP config and security validation
    models/                   # Domain structs
    runtime/                  # Session state, learners, triggers
    secrets.rs                # macOS Keychain integration
    services/                 # Domain logic and managed Ollama runtime
    speech/ and voice/        # Voice protocol and server TTS
    util/                     # SSE, capture, network and text utilities
  migrations/                 # SQLite schema (auto-run on startup)
  deny.toml                   # cargo-deny license/ban policy

BoBeMacUI/                    # Swift macOS app (BoBe.app)
  BoBe/App/                   # App delegate, overlay panel, tray
  BoBe/Features/Settings/     # Settings panels (AI model, behavior, etc.)
  BoBe/DTOs/ and BoBe/Stores/ # Wire contracts and presentation state
  BoBeUITests/                # macOS onboarding/settings regression tests
  BoBe/Services/              # Backend lifecycle, HTTP + SSE client
  BoBe/Stores/                # Observable state stores
  BoBe/Voice/                 # Microphone, STT/VAD/EOU, playback, voice protocol
  BoBe/Views/                 # Overlay UI + setup wizard

docs/                         # Additional documentation
```

## Coding Conventions

### Rust

- **Edition 2024**, MSRV 1.97.1, `unsafe_code = "deny"`
- **Clippy pedantic** enabled with justified allows (see `Cargo.toml`)
- Errors via `thiserror`, handlers return `Result<T, AppError>` — no `unwrap()`/`expect()` outside tests
- LLM prompts: per-class SKILL.md in `copilot/skills/` (loaded by the SDK via `SessionConfig::skill_directories`); inline system-message hints in `copilot/hooks.rs` and `copilot/workers/chat.rs`
- Configuration via `BOBE_*` env vars, persisted to `~/.bobe/config.toml`
- API keys stored in macOS Keychain via `security-framework`, handled in-memory with the `secrecy` crate
- Follow [docs/RUST_GUIDELINES.md](docs/RUST_GUIDELINES.md) for architecture and style

### Swift

- Swift 6.0, macOS 15+ target
- **SwiftLint** enforced (see `BoBeMacUI/.swiftlint.yml`)
- `sorted_imports` required, `force_unwrapping` discouraged
- Split large views into focused subviews

### General

- No global package installs — app dependencies stay in their manifests and
  developer CLIs stay under the checkout's ignored `.tools/` directory
- Commit messages: `type: short description` (e.g., `feat:`, `fix:`, `chore:`)

## Domain Model

BoBe persists:

- conversations and cooldowns in SQLite;
- souls and user profiles in SQLite;
- one editable `memory.md` document;
- human-readable goal documents under the data root;
- runtime and MCP configuration;
- worker session identifiers used to reconnect to Copilot sessions.

Retention is explicit only where the current code implements it; do not infer lifecycle promises from historical plans.

## API Reference

The backend exposes a REST API on `127.0.0.1:8766`:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/status` | GET | Runtime session state |
| `/events` | GET | SSE event stream (real-time updates) |
| `/message` | POST | Send a message to BoBe |
| `/goals` | GET/POST | List or create goals |
| `/goals/{id}` | GET/PATCH/DELETE | Goal CRUD |
| `/goals/{id}/complete`, `/goals/{id}/archive` | POST | Goal lifecycle actions |
| `/memory` | GET/PUT | Read or replace the memory document |
| `/souls` | GET/POST | Personality document management |
| `/user-profiles` | GET/POST | User profile management |
| `/settings` | GET/PATCH | Runtime configuration |
| `/models` | GET | List models for the selected engine |
| `/local-runtime/install`, `/local-runtime/cancel` | POST | Manage local runtime installation |
| `/local-runtime/status` | GET | Local runtime installation SSE |
| `/tools/mcp/config` | GET/PUT/DELETE | MCP server configuration |
| `/voice/stream` | GET | Voice WebSocket |
| `/voice/install/*` | GET/POST | Voice model installation |
| `/auth/copilot/login/*` | GET/POST | Copilot device-flow sign-in |
| `/privacy/data` | DELETE | Purge personal data |
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

For supply-chain-sensitive changes, run the local `cargo deny` and locked Cargo commands before review.

Release signing, notarization, Sparkle publishing, and validation are local maintainer operations. Maintainers should read [docs/UpdatingOTA.md](docs/UpdatingOTA.md) before changing release scripts, entitlements, or update behavior.

## Security

BoBe handles screen captures, personal context, and agent credentials. Please be mindful of:

- The daemon binds to loopback by default; remote mode is an explicit authenticated TLS deployment
- Host validation middleware on all routes
- Shell and arbitrary file-write permissions are denied; memory/goal changes use exact allowlisted domain tools
- MCP commands are validated against a blocklist
- Secrets go through the daemon secret-store abstraction and must never be logged
- Release signing, notarization, Sparkle, and update-host credentials must stay outside the repository

See [SECURITY.md](SECURITY.md) for our vulnerability reporting policy.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
