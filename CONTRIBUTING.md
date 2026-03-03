# Contributing to BoBe

Thanks for your interest in contributing! BoBe is an open-source project and we welcome contributions of all kinds.

## Getting Started

### Prerequisites

- **Rust 1.93+** (edition 2024)
- **[just](https://github.com/casey/just)** task runner
- **[Ollama](https://ollama.ai)** for local LLM inference (or an OpenAI/Azure API key)
- **macOS 14+ (Sonoma)** with Xcode 16+ for the Swift desktop app
- **SwiftLint** for Swift linting

### Setup

```bash
# Clone the repo
git clone https://github.com/user/bobe.git
cd bobe

# Check everything builds and passes
just check
```

`just check` runs: `cargo fmt --check`, `cargo clippy`, `cargo test`, `swiftlint`, and `swift build`.

### Build Commands

```bash
just run            # Debug build + launch app
just backend        # Backend only (for Xcode development)
just build          # Release build + bundle .app
just clean          # Clean all artifacts
just check          # Full lint + test suite
```

## Development Workflow

1. **Fork and branch** from `main`
2. **Make your changes** following the coding conventions below
3. **Run `just check`** to verify everything passes
4. **Submit a pull request** with a clear description

## Project Structure

```
src/                          # Rust backend (bobe-daemon)
  api/                        # Axum routes and handlers
  bootstrap/                  # Dependency wiring and startup
  config.rs                   # Configuration (BOBE_* env vars)
  config_manager/             # Runtime hot-swap config
  db/                         # SQLite repositories (sqlx)
  llm/                        # LLM provider abstraction
  models/                     # Domain structs
  runtime/                    # Session state, learners, triggers, prompts
  services/                   # Business logic layer
  tools/                      # Native tools + MCP integration
  util/                       # SSE, capture, tokens, text utils

desktopMac/                   # Swift macOS app (BoBe.app)
  BoBe/App/                   # App delegate, overlay panel, tray
  BoBe/Models/                # API DTOs, entity types
  BoBe/Services/              # Backend lifecycle, HTTP + SSE client
  BoBe/Stores/                # Observable state stores
  BoBe/Views/                 # Overlay, settings, setup wizard

migrations/                   # SQLite migrations (auto-run on startup)
docs/                         # Additional documentation
```

## Coding Conventions

### Rust

- **Edition 2024**, MSRV 1.93, `unsafe_code = "deny"`
- **Clippy pedantic** enabled with justified allows (see `Cargo.toml`)
- Errors via `thiserror`, handlers return `Result<T, AppError>` — no `unwrap()`/`expect()` outside tests
- LLM prompts live exclusively in `runtime/prompts/`
- Configuration via `BOBE_*` env vars, persisted to `~/.bobe/config.toml`
- API keys stored in macOS Keychain via the `secrecy` crate
- Follow [RUST_GUIDELINES.md](docs/RUST_GUIDELINES.md) for architecture and style

### Swift

- Swift 6.0, macOS 14+ target
- **SwiftLint** enforced (see `desktopMac/.swiftlint.yml`)
- `sorted_imports` required, `force_unwrapping` discouraged
- Split large views into focused subviews

### General

- Keep functions under 50 lines, files under 500 lines
- No global package installs — all dependencies stay in the project
- Commit messages: `type: short description` (e.g., `feat:`, `fix:`, `chore:`)

## Architecture Principles

- **Constructor injection** via `AppState` (Arc-wrapped, Axum State extractor) — no DI framework
- **Layered architecture**: Handler (thin) -> Service (logic) -> Repository (data)
- **Trait-based abstraction** for LLM providers, embedding, and repositories
- **Hot-swappable config** via `ArcSwap` — settings changes apply without restart
- **localhost-only** by design — all network traffic stays on `127.0.0.1`

## Security

BoBe handles screen captures and LLM API keys. Please be mindful of:

- All endpoints bind to `127.0.0.1` only — never expose to network
- Host validation middleware on all routes
- File tool access uses `canonicalize()` + ancestry checks
- MCP commands are validated against a blocklist
- API keys go through macOS Keychain, never logged or persisted in plaintext

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
