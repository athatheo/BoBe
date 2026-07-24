<h1 align="center">BoBe</h1>

<p align="center">
  <em>A local-first proactive AI companion for macOS</em>
  <br>
  <a href="https://www.bobebot.com">BoBeBot.com</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/platform-macOS%2015%2B-lightgrey?logo=apple" alt="macOS 15+">
  <img src="https://img.shields.io/badge/arch-Apple%20Silicon-orange" alt="Apple Silicon">
</p>

---

BoBe lives on your desktop as a transparent overlay. It watches what you're working on, builds memories over time, tracks your goals, and reaches out when it thinks it can help — like a thoughtful colleague who actually pays attention.

BoBe owns its data and runtime locally by default. You choose whether inference uses your GitHub Copilot subscription or a local Ollama model; only the context needed for a cloud answer leaves the Mac.

## What BoBe Does

| | |
|---|---|
| **Observes your work** | Periodic screen captures analyzed by a vision model to understand context |
| **Remembers you** | Maintains a local memory document; the agent can append durable facts through a bounded daemon tool |
| **Tracks your goals** | Keeps human-readable goals, creates them after confirmation, and uses active goals proactively |
| **Reaches out proactively** | A decision engine evaluates when help is valuable — not a chatbot waiting for input |
| **Respects your flow** | Cooldown logic and engagement awareness prevent interruptions |
| **Uses tools** | Read-only file access plus extensible [MCP](https://modelcontextprotocol.io/) server integration |
| **Customizable personality** | Soul documents shape how BoBe communicates |

## Supported Platforms

| Platform | Architecture | Minimum Version | Status |
|----------|-------------|-----------------|--------|
| macOS    | Apple Silicon (arm64) | macOS 15 Sequoia | ✅ Supported |
| Ubuntu daemon | x86_64 | — | 🧭 Compatibility target; no packaged release or continuous validation |

Mobile and physical-device surfaces are future platform directions, not shipped applications. See [docs/platform-architecture.md](docs/platform-architecture.md).

## Quick Start

### Option A: Download a Release

Download the latest `BoBe.dmg` from the [Releases](https://github.com/athatheo/BoBe/releases) page, drag BoBe to Applications, and launch it.

### Option B: Build from Source

```bash
git clone https://github.com/athatheo/BoBe.git
cd BoBe
just run
```

> Requires macOS 15+, Rust 1.97.1+, Xcode 16+, and [just](https://github.com/casey/just). See [CONTRIBUTING.md](CONTRIBUTING.md) for full prerequisites.

### First Launch

On first launch, BoBe's eight-stage setup wizard walks you through:

1. **Choose your AI** — GitHub Copilot cloud or local Ollama
2. **Authenticate or install** — sign in through BoBe's signed Copilot CLI helper, or let BoBe install Ollama and models
3. **Personalize** — tell BoBe your name and optionally create a first goal
4. **Choose proactivity** — tune how often BoBe observes and reaches out
5. **Grant permissions** — screen awareness is optional
6. **Prepare voice** — install the speech models used by the Mac

After setup, BoBe appears as a floating overlay on your desktop with a menu bar icon.

### LLM Providers

| Engine | Description |
|--------|-------------|
| **GitHub Copilot** | Cloud inference using the user's Copilot subscription and CLI authentication. |
| **[Ollama](https://ollama.ai)** | Local inference. BoBe can install the runtime and selected models. |

## Configuration

All settings are configurable through BoBe's settings panel (click the menu bar icon → Settings). Settings persist to `~/.bobe/config.toml`.

Environment variable overrides are available for advanced use:

```bash
BOBE_ENGINE__ENGINE=local
BOBE_CAPTURE__ENABLED=false
BOBE_CAPTURE__INTERVAL_SECONDS=30
```

Data is stored under `~/.bobe/` by default. Set `BOBE_DATA_DIR` to relocate the canonical data root; `BOBE_DATABASE__URL` remains available as an explicit SQLite override.

## Security

BoBe handles sensitive data including screen captures, personal context, and integration secrets. See [SECURITY.md](SECURITY.md) for our vulnerability reporting policy.

Key security properties:

- **Loopback by default** — the daemon binds to `127.0.0.1` unless remote mode is deliberately configured
- **Secrets** stored in macOS Keychain and handled in-memory with the `secrecy` crate
- **Arbitrary file writes and shell commands** are denied; memory/goal mutations use bounded daemon-owned tools
- **MCP commands** validated against a configurable blocklist
- **CORS** locked to localhost origins
- **Remote/reverse-proxy access** must configure an API bearer token and TLS certificate/key; adding a public allowed host without both is rejected at startup, including when the daemon itself binds to loopback behind a proxy

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, architecture, coding conventions, and the full project structure.

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## License

[MIT License](LICENSE)
