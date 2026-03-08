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

`just check` runs: `cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo deny check`, `cargo machete`, `swiftlint`, and `swift build`.

`just check-ci` is the stricter CI-facing variant: it uses `--locked` for Cargo resolution and runs `cargo vet --locked`.

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

### Dependency Review Expectations

Dependency-changing pull requests get extra scrutiny. Call out any new or materially changed:

- proc-macro crates,
- `build.rs` crates,
- `-sys` / FFI crates,
- git dependencies,
- new registries,
- crates with broad network, filesystem, archive, parser, or subprocess reach.

For supply-chain-sensitive changes, expect CI to enforce `cargo vet`, `cargo deny`, and deterministic Cargo resolution.

### CI and Release Model

BoBe uses two lanes:

- **Public vetting CI**: PR/push validation with no release secrets.
- **Protected release workflow**: macOS signing, notarization, Sparkle signing, and update publishing.

Release secrets must never be used in normal CI. Workflow files, release scripts, OTA docs, and entitlements should be reviewed carefully because they sit on the release control plane.

Maintainers should read [docs/UpdatingOTA.md](docs/UpdatingOTA.md) before changing CI, release scripts, signing, notarization, or Sparkle publishing behavior.

## Project Structure

```
src/                          # Rust backend (bobe-daemon)
  api/                        # Axum routes and handlers
  app_state.rs                # Arc-wrapped DI container
  binary_manager/             # Ollama binary download/extraction
  bootstrap/                  # Dependency wiring and startup
  config.rs                   # Configuration (BOBE_* env vars)
  config_manager/             # Runtime hot-swap config
  db/                         # SQLite repositories (sqlx)
  i18n/                       # Internationalization (fluent)
  llm/                        # LLM provider abstraction
  models/                     # Domain structs
  runtime/                    # Session state, learners, triggers, prompts
  secrets.rs                  # macOS Keychain integration
  services/                   # Business logic layer
  tools/                      # Native tools + MCP integration
  util/                       # SSE, capture, tokens, text utils

desktopMac/                   # Swift macOS app (BoBe.app)
  BoBe/App/                   # App delegate, overlay panel, tray
  BoBe/Features/Settings/     # Settings panels (AI model, behavior, etc.)
  BoBe/Models/                # API DTOs, entity types
  BoBe/Services/              # Backend lifecycle, HTTP + SSE client
  BoBe/Stores/                # Observable state stores
  BoBe/Theme/                 # Theme configuration
  BoBe/Views/                 # Overlay UI + setup wizard

migrations/                   # SQLite schema (auto-run on startup)
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
- **Layered architecture**: Handler -> Service -> Repository (some simple handlers call repos directly)
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
- Release signing, notarization, Sparkle, and update-host credentials belong only in protected CI environments

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
