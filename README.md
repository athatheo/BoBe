<h1 align="center">BoBe</h1>

<p align="center">
  <em>A local-first proactive AI companion for macOS</em>
  <br>
  <a href="https://www.bobebot.com">BoBeBot.com</a>
</p>

<p align="center">
  <a href="https://github.com/athatheo/BoBe/actions/workflows/ci.yml"><img src="https://github.com/athatheo/BoBe/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://github.com/athatheo/BoBe/actions/workflows/release.yml"><img src="https://github.com/athatheo/BoBe/actions/workflows/release.yml/badge.svg?event=workflow_dispatch" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/platform-macOS%2014%2B-lightgrey?logo=apple" alt="macOS 14+">
  <img src="https://img.shields.io/badge/arch-Apple%20Silicon-orange" alt="Apple Silicon">
</p>

---

BoBe lives on your desktop as a transparent overlay. It watches what you're working on, builds memories over time, tracks your goals, and reaches out when it thinks it can help — like a thoughtful colleague who actually pays attention.

Everything runs locally by default. Your data never leaves your machine unless you choose a cloud LLM provider.

## What BoBe Does

| | |
|---|---|
| **Observes your work** | Periodic screen captures analyzed by a vision model to understand context |
| **Remembers you** | Short-term and long-term memory from conversations and observations |
| **Tracks your goals** | Extracts goals from conversation, persists them, references them proactively |
| **Reaches out proactively** | A decision engine evaluates when help is valuable — not a chatbot waiting for input |
| **Respects your flow** | Cooldown logic and engagement awareness prevent interruptions |
| **Uses tools** | File access, memory search, and extensible [MCP](https://modelcontextprotocol.io/) server integration |
| **Customizable personality** | Soul documents shape how BoBe communicates |

## Supported Platforms

| Platform | Architecture | Minimum Version | Status |
|----------|-------------|-----------------|--------|
| macOS    | Apple Silicon (arm64) | macOS 14 Sonoma | ✅ Supported |
| Windows  | x86_64 / arm64 | — | 🚧 TBD |

> **Note:** Linux and iOS are not currently planned.

## Quick Start

### Option A: Download a Release

Download the latest `BoBe.dmg` from the [Releases](https://github.com/athatheo/BoBe/releases) page, drag BoBe to Applications, and launch it.

### Option B: Build from Source

```bash
git clone https://github.com/athatheo/BoBe.git
cd Bobe
just run
```

> Requires Rust 1.93+, Xcode 16+, and [just](https://github.com/casey/just). See [CONTRIBUTING.md](CONTRIBUTING.md) for full prerequisites.

### First Launch

On first launch, BoBe's setup wizard walks you through:

1. **Choose your AI** — local (Ollama, runs on your Mac) or cloud (OpenAI / Azure)
2. **Local setup** — BoBe downloads and manages a local Ollama installation + models automatically
3. **Cloud setup** — paste your API key and pick a model
4. **Screen awareness** (optional) — grant Screen Recording permission so BoBe can observe what you're working on

After setup, BoBe appears as a floating overlay on your desktop with a menu bar icon.

### LLM Providers

| Provider | Description |
|----------|-------------|
| **[Ollama](https://ollama.ai)** | Recommended for local inference. BoBe manages the Ollama installation for you. |
| **OpenAI** | Cloud inference (GPT-5 family). Requires an API key. |
| **Azure OpenAI** | Enterprise cloud inference. Requires endpoint, key, and deployment name. |
| **llama.cpp** | Direct local inference without Ollama. |

## Configuration

All settings are configurable through BoBe's settings panel (click the menu bar icon → Settings). Settings persist to `~/.bobe/config.toml`.

Environment variable overrides are available for advanced use:

```bash
BOBE_LLM__BACKEND=openai
BOBE_CAPTURE__ENABLED=false
BOBE_CAPTURE__INTERVAL_SECONDS=30
```

Data is stored at `~/.bobe/` — SQLite database, configuration, goals, and MCP server config.

## Security

BoBe handles sensitive data including screen captures and API keys. See [SECURITY.md](SECURITY.md) for our vulnerability reporting policy.

Key security properties:

- **Localhost only** — all endpoints bind to `127.0.0.1`, never exposed to the network
- **API keys** stored in macOS Keychain, handled in-memory with the `secrecy` crate
- **File tools** use path canonicalization + ancestry checks
- **MCP commands** validated against a configurable blocklist
- **CORS** locked to localhost origins

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, architecture, coding conventions, and the full project structure.

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## License

[MIT License](LICENSE)
