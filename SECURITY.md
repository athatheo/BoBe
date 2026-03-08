# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| Latest release | ✅ |
| Older releases | ❌ |

Only the latest release receives security updates.

## Reporting a Vulnerability

**Please do not open a public issue for security vulnerabilities.**

Instead, report them privately via **[GitHub Security Advisories](https://github.com/johnkozaris/Bobe/security/advisories/new)**.

Include:

- A description of the vulnerability
- Steps to reproduce or a proof-of-concept
- The impact you've assessed
- Any suggested fix (optional)

We will acknowledge receipt within 48 hours and aim to provide a fix or mitigation plan within 7 days for confirmed vulnerabilities.

## Scope

The following are in scope:

- **Rust backend** (`src/`) — API handlers, LLM orchestration, tool execution, file access, config/secrets handling
- **Swift frontend** (`BoBeMacUI/`) — backend communication, credential handling
- **Build and release pipeline** — CI workflows, code signing, notarization, update distribution
- **MCP server integration** — command validation, environment handling

The following are out of scope:

- Vulnerabilities in upstream dependencies (report those to the upstream project)
- Issues requiring physical access to the machine
- Social engineering attacks

## Security Design

BoBe is designed with the following security properties:

- All network traffic is bound to `127.0.0.1` — the backend is never exposed to the network
- Host validation middleware on every route
- API keys are stored in the macOS Keychain and handled in-memory with the `secrecy` crate
- File tool access uses `canonicalize()` + ancestry checks to prevent path traversal
- MCP commands are validated against a configurable blocklist
- CORS is locked to localhost origins
