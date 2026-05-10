# Engine / provider notes — local models, bundling, onboarding rework

> **Status:** decisions ratified, Phase 0 ready to land. Phases 1–3 scoped
> below for follow-up commits.
> **Branch context:** continuation of `feat/copilot-sdk-pivot`.
> **Date:** 2026-05-10.

---

## TL;DR

1. We keep `github-copilot-sdk` as the only agent-loop SDK. No `pi_agent_rust` (one-contributor risk), no Goose (not on crates.io), no OpenCode (different topology, less local-friendly).
2. **Local models are supported via Copilot CLI BYOK** (shipped by GitHub on 2026-04-07) — set `COPILOT_PROVIDER_BASE_URL`, `COPILOT_MODEL`, etc. before spawning the SDK Client. Same agent loop, different env.
3. **The Copilot CLI is bundled in BoBe** via `embedded-cli` feature + `COPILOT_CLI_VERSION` build env. Drops the wizard's "is copilot installed?" branch and the user's responsibility to install anything.
4. **Local model runtime: Ollama, downloaded on demand during onboarding.** Not bundled in the .app — the binary is fetched at first-launch when the user picks the Local-AI path, models are pulled the same way. BoBe owns the download + spawn lifecycle (resurrects the pre-pivot `binary_manager` + `ollama_manager` modules from main). Pi, OpenCode, Aider, and Goose all default to Ollama in their first-run docs — matching that muscle memory is part of why we pick it. If the user already has `ollama serve` on `127.0.0.1:11434`, BoBe detects and uses theirs (no second copy). After the runtime + models are ready, BoBe just sets `COPILOT_PROVIDER_BASE_URL` env vars on its Copilot CLI subprocesses; the SDK handles every actual call to Ollama from there. FFI-into-llama.cpp was considered and rejected — TCP loopback is ~50 µs, irrelevant vs seconds of inference.
5. **Onboarding reworks** to surface the engine choice up front (cloud Copilot vs local) and verify the picked path actually works.

---

## What we discovered

### Copilot CLI BYOK (April 7, 2026)

The CLI now respects `COPILOT_PROVIDER_*` env vars to redirect inference to any OpenAI-compatible endpoint. Confirmed working with Ollama, vLLM, LM Studio, llama.cpp's `llama-server`, Foundry Local. Same Rust SDK on our side — only the spawned subprocess sees the redirect.

```bash
# Local llama.cpp example
COPILOT_PROVIDER_BASE_URL=http://localhost:8080/v1
COPILOT_MODEL=Qwen3-8B
COPILOT_OFFLINE=true                      # full air-gap, kills GitHub telemetry
# DO NOT set COPILOT_PROVIDER_API_KEY for local — empty key triggers failures
```

Sources: [GitHub Changelog](https://github.blog/changelog/2026-04-07-copilot-cli-now-supports-byok-and-local-models/), [GitHub Docs: BYOK](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-byok-models), [Ollama Copilot CLI integration](https://docs.ollama.com/integrations/copilot-cli).

### CLI bundling in `github-copilot-sdk` Rust

The Rust SDK has `embedded-cli` feature (uses `sha2` + `zstd` deps). When `COPILOT_CLI_VERSION` is set at build time:

1. `build.rs` downloads `copilot-{platform}-{arch}.tar.gz` from `github.com/github/copilot-cli/releases/download/v{VERSION}/`
2. Verifies SHA-256 against the matching `SHA256SUMS.txt`
3. zstd-19 compresses the binary (~15–30 MB compressed)
4. Embeds via `include_bytes!` into the crate
5. Sets `cfg(has_bundled_cli)` so the runtime resolver activates

Resolution order at runtime ([`rust/src/resolve.rs`](https://github.com/github/copilot-sdk/blob/main/rust/src/resolve.rs)):

1. `COPILOT_CLI_PATH` (explicit override)
2. **Bundled binary** — extracted lazily to `~/.cache/github-copilot-sdk-{ver}/copilot` on first call, SHA-256 verified, cached in `OnceLock`
3. PATH lookup + standard install dirs (homebrew, fnm, nvm, volta, cargo, etc.)

Targets: macos-arm64, macos-x64, linux-x64, linux-arm64, windows-x64, windows-arm64.

Source: [github/copilot-sdk#248 (Steve Sanderson's confirmation)](https://github.com/github/copilot-sdk/issues/248#issuecomment-3813716744).

### What we're NOT doing — and why

- **Not switching SDKs.** `pi_agent_rust` is a single-contributor port (873 stars, 1 maintainer). Goose isn't published to crates.io (name collision with the load-testing tool), would require git-dep or vendoring. OpenCode is HTTP+SSE topology, different shape, weaker local-model story per Reddit sentiment. Sticking with `github-copilot-sdk` keeps a single agent loop, single set of code paths, single maintenance burden — and now-bundled means no install friction.
- **Not building an in-process FFI-to-llama.cpp shim.** The Copilot CLI subprocess can only speak HTTP to its inference backend. Loopback HTTP is ~50 µs vs ~seconds of inference (irrelevant). The "FFI shim" only saves one external process to manage, at the cost of a model-management subsystem in BoBe + cmake-llama.cpp build complexity + macOS notarization friction. Net negative.
- **Not abandoning the Copilot agent loop in favor of direct llama.cpp FFI.** Doing so would mean re-implementing skills, hooks, autopilot, MCP server lifecycle, tool dispatch, JSON repair, multi-turn iteration — exactly the `LlmProvider` infrastructure we deleted during the pivot.
- **Not bundling Ollama in the .app.** The macOS Ollama is small (~100–300 MB) so it could fit, but bundling complicates code-signing/notarization (third-party binary inside our bundle), increases the download we ship, and forces lockstep version pinning. Downloading at first-launch (with progress UI) is the same pattern the old wizard used pre-pivot and matches how Ollama's own update mechanism works. If the user already has Ollama installed/running, BoBe detects it and skips the download entirely.
- **Not bundling `llama-server` + `llama-swap`** despite tempting size. We'd be writing our own Ollama (model download + GGUF cache + multi-model scheduler) for marginal speed gain on a workload where it's irrelevant. Ollama already does the boring stuff well.
- **Not bundling LlamaEdge.** Smallest at 30 MB but no native multi-model, less mature ecosystem, requires `llama-swap` on top for hot-swap. Niche fit, not ours.
- **Not proxying/wrapping Ollama calls inside BoBe.** BoBe sets the `COPILOT_PROVIDER_BASE_URL` env at Copilot CLI Client construction; the SDK + spawned CLI subprocess handle every model call directly. BoBe never sits in the request path between the CLI and Ollama. Less code, fewer failure modes.

---

## Architecture (final shape)

```
┌──────────────────────────────────────────────────────────────────┐
│ BoBe daemon (Rust binary, ~120 MB after stripping)               │
│                                                                  │
│   github-copilot-sdk (with embedded-cli feature, Phase 0)        │
│   ├─ extracts copilot CLI to ~/.cache/github-copilot-sdk-{ver}/  │
│   └─ spawns 2× CLI subprocesses (text-client + vision-client)    │
│                                                                  │
│   binary_manager (resurrected from main, simplified)             │
│   ├─ ensures Ollama binary at ~/.bobe/runtimes/ollama            │
│   │   (downloads if absent, validates, signs is unchanged)       │
│   └─ progress streamed via watch::Sender<DownloadProgress>       │
│                                                                  │
│   ollama_manager (resurrected from main)                         │
│   ├─ spawns `ollama serve` (or skips if user's runs on :11434)   │
│   ├─ ollama pull qwen2.5:7b-instruct (text)                      │
│   └─ ollama pull qwen2.5-vl:7b (vision)                          │
│                                                                  │
└──┬───────────────────────────────────────────────────────────────┘
   │ stdio JSON-RPC                          (sets up ↑ on choice)
   ▼
┌─────────────────────────────────┐
│ 2× copilot CLI subprocesses     │  COPILOT_PROVIDER_BASE_URL =
│ Different COPILOT_MODEL each:   │      http://127.0.0.1:11434/v1
│  text-client:  qwen2.5:7b       │ ──────────┐
│  vision-client: qwen2.5-vl:7b   │            ▼
└──┬──────────────────────────────┘    ┌────────────────────────┐
   │ HTTPS (cloud)         (local)     │ ollama serve           │
   ▼                                   │ Native multi-model     │
GitHub Copilot                         │ OLLAMA_MAX_LOADED=2    │
(cloud mode — no Ollama)               │ Models in ~/.ollama/   │
                                       └────────────────────────┘
                                       (NOT in BoBe's request path —
                                        SDK + CLI handle calls direct)
```

The daemon is small (~120 MB final binary; Phase 0's bundled CLI dominates). The Ollama runtime + ~10 GB of model weights live outside the bundle, downloaded on demand. If the user already runs Ollama (their existing `~/.ollama/models` is shared so any models they previously pulled are detected), BoBe skips the runtime download entirely — only the missing models are pulled.

---

## Phased implementation plan

### Phase 0 — Bundling (small, ready to land)

**Files:**
- `BoBeService/Cargo.toml` — `github-copilot-sdk = { version = "0.1", features = ["embedded-cli"] }`
- `BoBeService/.cargo/config.toml` (new) — `[env] COPILOT_CLI_VERSION = "0.0.341"` (or pinned current)

**Effects:**
- BoBe binary grows ~15–30 MB
- First app launch extracts to `~/.cache/github-copilot-sdk-{ver}/`
- Wizard step 2 ("is `copilot` installed?") becomes always-true; can be simplified or removed
- macOS notarization needs verification on a signed build (extracted binary, codesign)

**Tests:**
- Clean Mac without Copilot CLI installed: BoBe starts, daemon spawns CLI from extracted cache
- `swift build` + `cargo build` green
- `just check` clean

### Phase 1 — Daemon adds engine + provider fields to settings

`SettingsResponse` / `SettingsUpdateRequest` (Rust + Swift) gain:

```rust
pub(crate) struct SettingsResponse {
    // ... existing 9 fields ...
    pub(crate) engine: String,                          // "copilot_cloud" | "local"
    pub(crate) provider_base_url: Option<String>,       // local: typically http://127.0.0.1:11434/v1
    pub(crate) provider_text_model: Option<String>,     // local: qwen2.5:7b-instruct
    pub(crate) provider_vision_model: Option<String>,   // local: qwen2.5-vl:7b
    pub(crate) provider_offline: bool,                  // local: usually true
}
```

Daemon constructs **two Copilot clients in local mode** so vision/text can use different models (BYOK fixes `COPILOT_MODEL` per-process). Cloud mode keeps a single client.

```rust
match cfg.engine.as_str() {
    "local" => {
        let base = cfg.provider_base_url.as_deref().unwrap_or("http://127.0.0.1:11434/v1");
        let text_client = Client::start(opts_with_env(&[
            ("COPILOT_PROVIDER_BASE_URL", base),
            ("COPILOT_MODEL", cfg.provider_text_model.as_deref().unwrap_or("qwen2.5:7b-instruct")),
            ("COPILOT_OFFLINE", if cfg.provider_offline { "true" } else { "false" }),
        ])).await?;
        let vision_client = Client::start(opts_with_env(&[
            ("COPILOT_PROVIDER_BASE_URL", base),
            ("COPILOT_MODEL", cfg.provider_vision_model.as_deref().unwrap_or("qwen2.5-vl:7b")),
            ("COPILOT_OFFLINE", "true"),
        ])).await?;
    }
    _ => { /* cloud — single client, no env override */ }
}
```

`engine` change is **restart-required** (CLIs capture env at boot). UI restart-banner shadow set extends with `["engine", "provider_base_url", "provider_text_model", "provider_vision_model", "provider_offline"]`.

### Phase 2 — Onboarding wizard rework

**New flow (5 steps; step 2 has 2 branches):**

```
1. Welcome
2. Engine choice — "How should BoBe think?"
   ├─ Cloud (GitHub Copilot)            ← needs subscription + auth
   └─ Local model                        ← needs local server running
3a. (if cloud) Auth check
   ├─ Authed   → "Ready ✓"
   └─ Not authed → "Sign in to GitHub Copilot" button, opens browser/terminal
3b. (if local) Server check
   ├─ Detected llama-server :8080 / Ollama :11434 / LM Studio :1234 → "Ready ✓"
   └─ None found → "Install one of these:" with three install commands + retry
4. Permissions (screen recording) — unchanged
5. Done
```

#### Step 2 copy

> **How should BoBe think?**
>
> **Cloud (GitHub Copilot)** — fast, smart, requires a Copilot subscription, prompts and screen captures go to GitHub.
>
> **Local model** — fully private, runs on your Mac, requires a local LLM server (we'll detect it). 32 GB+ unified memory recommended for usable speed.

Privacy framing inline in the choice; no separate "data residency" marketing copy needed.

#### Step 3a: Auth verification (cloud)

Run a no-op SDK call (e.g. `client.ping()` or `client.list_models()`). If it fails with auth-related error, surface "Sign in" with two buttons:
- **Open Terminal** — runs `copilot --login` (or whatever the CLI's auth subcommand is — verify in CLI docs)
- **Skip for now** — proceed to permissions; daemon will degrade gracefully

#### Step 3b: Local server detection (local)

Probe sequentially in BoBe's daemon (returns to wizard via daemon-state event):
1. `GET http://localhost:11434/api/version` — Ollama
2. `GET http://localhost:8080/health` — llama-server
3. `GET http://localhost:1234/v1/models` — LM Studio

If any responds: surface "Detected: <runtime>", auto-fill `provider_base_url`. Show a model picker if `/v1/models` listing succeeds.

If none: show "Install one:" with three preferred commands:
- `brew install llama.cpp && llama-server -hf Qwen/Qwen3-8B-GGUF:Q8_0 --jinja -c 65536`
- `brew install ollama && ollama serve & ollama pull qwen3:8b`
- LM Studio download link

Both options show **hardware warning** if `sysctl -n hw.memsize` returns < 32 GB.

#### Step 4: Permissions (unchanged from current wizard)

#### Step 5: Done

Same as current.

### Phase 5 — Resurrect download flow for Ollama + models

Bring back (simplified) the pre-pivot infrastructure. From the `main` branch:
- `BoBeService/src/binary_manager/{mod,download,extract}.rs` — ~460 lines, downloads Ollama from official release with SHA verification + progress events
- `BoBeService/src/services/ollama_runtime_service.rs` — coordinates download + spawn lifecycle
- `BoBeService/src/llm/ollama_manager.rs` — `ensure_daemon_running`, `ollama pull` orchestration
- `BoBeMacUI/BoBe/Views/Setup/SetupWizardProgressViews.swift` — download progress UI

What changes vs. the old version:
- Drop the LLM-provider abstraction entirely — Ollama is the only target. No Azure / OpenAI / llama.cpp branches.
- BoBe doesn't proxy Ollama HTTP calls — daemon's only job is "is the runtime ready, are the models pulled, are env vars wired into the Copilot CLI". Once those are true, BoBe is out of the request path.
- Pull both models concurrently with progress events (`text_progress` + `vision_progress`), not sequentially.
- Skip download entirely if `127.0.0.1:11434` is already responding (user has their own Ollama).

API for the wizard:
- `POST /local-runtime/install` → starts download, returns immediately
- `GET /local-runtime/status` (SSE) → streams `{stage: "ollama"|"text-model"|"vision-model", percent, bytes_done, bytes_total}` events
- `POST /local-runtime/cancel` → aborts an in-flight download

Wizard subscribes to the SSE stream, renders three stacked progress bars. On completion, settings are updated with the model names and `engine: "local"` and the wizard advances.

Defaults (subject to refinement):
- Text model: `qwen2.5:7b-instruct` (~5 GB Q4_K_M)
- Vision model: `qwen2.5-vl:7b` (~5 GB Q4_K_M)
- Total first-run download: ~250 MB Ollama binary + ~10 GB models = ~10.25 GB
- Hardware floor: warn if `sysctl hw.memsize` returns < 16 GB; recommend 32 GB+

If user already has Ollama running, only the missing models pulled (BoBe checks `GET /api/tags` before issuing pulls).

### Phase 3 — Settings adds an Engine section

New top-level Settings category `Engine` (or fold into existing `Behavior` / `Advanced`).

Surface:
- Radio: Cloud / Local
- (If local) Provider base URL field (auto-detected, editable)
- (If local) Model name field (auto-detected from /v1/models, or freeform)
- (Local) Offline toggle (binds to `COPILOT_OFFLINE`)
- (Cloud) "Sign in to GitHub Copilot" button (only shown if not authed)
- Restart-required banner (engine changes are restart-required)

### Phase 4 — Migration for returning users

Existing users have `bobe.onboarding_completed = true` from the previous wizard. After the rework lands, the flag stays true → they don't see the new wizard. We need:

- A non-blocking pill in the overlay if `engine` setting is unset or invalid: "Configure your engine in Settings"
- Or auto-default `engine = "copilot_cloud"` so things just keep working as before for returning users

Recommended: auto-default to `copilot_cloud` (matches today's behavior). Only nudge if the user actively turns on a feature that needs an engine choice (e.g. enables `mcp_enabled` or chooses a local model in advanced settings).

---

## Open questions

- **Vision model parity for local mode.** Capture loop expects vision capability. Most local Qwen / Llama text models don't have vision. Options:
  - Auto-disable capture when `engine == "local"` and vision-capable model isn't detected
  - Add a separate `vision_provider_base_url` and let user point to a vision model (e.g. Qwen-VL)
  - Skip vision in local mode, let user opt back in
  Probably defer until we see real usage patterns.

- **Subscription detection.** No clean way to programmatically detect "user has Copilot subscription" pre-auth. Best we can do: warn in step 2's copy, fail gracefully on first call if no subscription.

- **Auth flow specifics.** Copilot CLI auth is browser-based. Need to verify what the SDK exposes for triggering it. May require shell-out to `copilot login` (or whatever) in a Terminal app. Document the exact command.

- **CLI version pinning policy.** Bumping `COPILOT_CLI_VERSION` is a deliberate action. Track upstream releases and pin to stable channels. Consider adding to dependabot or similar.

- **Caching directory cleanup.** When BoBe upgrades and pins a new Copilot CLI version, the old cache dir lingers (`~/.cache/github-copilot-sdk-{old-ver}/`). Worth a one-time cleanup on first run after upgrade.

---

## Sources / references

- [GitHub Changelog: Copilot CLI BYOK (Apr 7, 2026)](https://github.blog/changelog/2026-04-07-copilot-cli-now-supports-byok-and-local-models/)
- [GitHub Docs: BYOK env vars](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-byok-models)
- [github/copilot-sdk issue #248](https://github.com/github/copilot-sdk/issues/248) and [Steve Sanderson's bundling confirmation](https://github.com/github/copilot-sdk/issues/248#issuecomment-3813716744)
- [`rust/build.rs`](https://github.com/github/copilot-sdk/blob/main/rust/build.rs)
- [`rust/src/embeddedcli.rs`](https://github.com/github/copilot-sdk/blob/main/rust/src/embeddedcli.rs)
- [`rust/src/resolve.rs`](https://github.com/github/copilot-sdk/blob/main/rust/src/resolve.rs)
- [Ollama Copilot CLI integration](https://docs.ollama.com/integrations/copilot-cli)
- [llama.cpp function-calling docs](https://github.com/ggml-org/llama.cpp/blob/master/docs/function-calling.md)
- Reddit/HN sentiment synthesis: see chat history with `grigio.org`, `mariozechner.at`, `thoughts.jock.pl` references
