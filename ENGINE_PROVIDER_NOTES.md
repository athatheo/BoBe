# Engine + Provider Plan — discoveries, decisions, and phased rollout

> **Branch:** `feat/copilot-sdk-pivot`
> **Date:** 2026-05-10 (updated continuously)
> **Status:** Phases 0, 1, and the wizard cards portion of Phase 2 have shipped on this branch. Phases 3, 4, 5, and the auth-check portion of Phase 2 are scoped below.

---

## Quick links

- [TL;DR](#tldr)
- [Discoveries](#discoveries-everything-we-learned-this-session)
- [Architectural decisions](#architectural-decisions)
- [What we rejected and why](#what-we-rejected-and-why)
- [Final architecture](#final-architecture)
- [Phased plan](#phased-plan)
- [Default models + hardware floor](#default-models--hardware-floor)
- [Open questions and risks](#open-questions--risks)
- [Sources](#sources)

---

## TL;DR

1. **One agent loop**: stay on `github-copilot-sdk` (Rust). It's bundled into BoBe via the `embedded-cli` feature so users don't need to install Copilot CLI separately. *No second SDK* (rejected `pi_agent_rust`, `goose`, `opencode-sdk-rs`, `claude-agent-sdk-rs`).
2. **Two engine modes**: cloud (GitHub Copilot subscription) and local (user-managed Ollama). User picks at first-launch in the wizard; configurable later in Settings. Restart-required.
3. **Local runtime is downloaded, not bundled**: Ollama is fetched on first local-mode use (~250 MB binary), Qwen 2.5 7B + Qwen 2.5-VL 7B models pulled the same way (~10 GB total). Existing Ollama on `:11434` is detected and reused.
4. **Vision via two Copilot CLI subprocesses in local mode**: BYOK fixes `COPILOT_MODEL` at spawn, so we run text-client (Qwen 2.5) and vision-client (Qwen 2.5-VL) in parallel. Both point at the same Ollama; Ollama routes by `model` field.
5. **BoBe never sits in the inference request path**: daemon's only job is wiring `COPILOT_PROVIDER_BASE_URL` env at Client construction. SDK + spawned CLI handle every actual model call. No FFI shim, no proxy, no MITM.
6. **Image attachments stay in-memory** via `Attachment::Blob` (base64 in JSON-RPC) — already optimal. No disk roundtrip.

---

## Discoveries (everything we learned this session)

### About `github-copilot-sdk` (the Rust crate at `github/copilot-sdk`)

- **BYOK shipped April 7, 2026.** Copilot CLI now respects `COPILOT_PROVIDER_BASE_URL`, `COPILOT_MODEL`, `COPILOT_PROVIDER_TYPE`, `COPILOT_PROVIDER_API_KEY`, `COPILOT_PROVIDER_WIRE_API`, `COPILOT_OFFLINE`, `COPILOT_PROVIDER_MAX_PROMPT_TOKENS`, `COPILOT_PROVIDER_MAX_OUTPUT_TOKENS`. Same Rust SDK on our side; only the spawned CLI subprocess sees the redirect. ([changelog](https://github.blog/changelog/2026-04-07-copilot-cli-now-supports-byok-and-local-models/), [docs](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-byok-models))
- **The `embedded-cli` feature bundles the CLI binary at build time.** When `COPILOT_CLI_VERSION` is set, the SDK's `build.rs` downloads `copilot-{platform}-{arch}.{tar.gz|zip}` from GitHub releases, verifies SHA-256, zstd-19 compresses, and `include_bytes!`s it. At runtime, `embeddedcli::path()` lazily extracts to `~/.cache/github-copilot-sdk-{ver}/copilot`, hash-verified, cached in a `OnceLock`. Resolve order: `COPILOT_CLI_PATH` env → bundled → PATH search. ([issue #248](https://github.com/github/copilot-sdk/issues/248), [Steve Sanderson confirm](https://github.com/github/copilot-sdk/issues/248#issuecomment-3813716744), [build.rs](https://github.com/github/copilot-sdk/blob/main/rust/build.rs), [resolve.rs](https://github.com/github/copilot-sdk/blob/main/rust/src/resolve.rs))
- **`client.get_auth_status() → GetAuthStatusResponse{is_authenticated, auth_type, host, login, status_message}`**. Lets us probe Copilot auth state programmatically. `ClientOptions::use_logged_in_user` defaults to `true` when no token is provided.
- **`Attachment::{File, Directory, Selection, Blob}`**: full attachment surface. `Blob{ data: base64, mime_type, display_name }` is what BoBe already uses for vision; no need to switch to `File{path}` (the disk roundtrip would be a regression — current code is already optimal).
- **The CLI subprocess only speaks HTTP/HTTPS** for inference — no UDS, no named pipes, no shared memory. `COPILOT_PROVIDER_BASE_URL` accepts only `http://` and `https://` schemes. We can't bypass HTTP between the CLI and the model.
- **Subprocess RAM cost**: Copilot CLI is Node.js, ~214 MB resident per process. Two Clients in local mode = ~430 MB just for the orchestrators.
- **CLI system prompt is ~21K tokens** (tool definitions). Local models need ≥64K context window for headroom; <32K causes silent truncation.

### About local-runtime alternatives we evaluated

| Tool | Bundle size (macOS) | Multi-model | Hot-swap | Community default for | Notes |
|---|---|---|---|---|---|
| **Ollama** | ~100–300 MB | **native (`OLLAMA_MAX_LOADED_MODELS`)** | **native** | **Pi, OpenCode, Aider, Goose** all default to it | de facto standard; 6–30% perf overhead vs raw llama.cpp |
| llama-server (raw) | ~30 MB | router mode (one resident, 3–10 s swap) | needs `llama-swap` | secondary | reference impl, fastest |
| llama-server + llama-swap | ~40 MB | yes via groups | yes | niche | we'd be reinventing Ollama |
| LlamaEdge | ~30 MB | one model per WASM instance | needs `llama-swap` | niche | WebAssembly, less mature |
| `llama-cpp-2` (Rust FFI) | ~90 MB linked | one model per ctx | manual | niche | requires cmake + Metal toolchain in BoBe build |
| **Ollama on macOS isn't 4.6 GB** — that figure was a Windows install with CUDA/ROCm libs we'd never use. macOS install is small enough to bundle but ([decision](#what-we-rejected-and-why)) we still don't.

### About Vision-Language models at 7B scale

- **Single VL-model approach loses text quality at small sizes.** Qwen 2.5-VL 7B is excellent at vision but trails dedicated text-only Qwen 2.5 7B on pure-text benchmarks. The gap closes at 32B+ (where Qwen3-VL-235B-A22B matches text-only equivalents on lmarena.ai). For BoBe's 7B-class default, two-model architecture is the right call.
- **Qwen 2.5/3 VL handles text-only requests cleanly**: special tokens separate vision from text inputs, so passing a text-only prompt doesn't engage the vision encoder. vLLM exposes `--limit-mm-per-prompt.image 0` for explicit text-only mode that frees the vision encoder's KV cache memory.
- **Recommended quantization**: Q4_K_M for the 7B class — ~5 GB on disk, sweet spot for quality vs memory.

### About hot-swapping models

- **`llama-server` router mode** (since mid-2024): `--models models.ini`. **Only one resident at a time** — switch costs 3–10 s.
- **`llama-swap`** ([repo](https://github.com/mostlygeek/llama-swap), 3K+ stars): purpose-built proxy, hot-swaps backends by `model` field in the request, `groups` feature for actually-concurrent residency.
- **Ollama**: native multi-model with smart memory scheduler — `OLLAMA_MAX_LOADED_MODELS=2` keeps both resident if RAM allows. Auto-unloads to fit.
- **Direct two `llama-server` processes on different ports**: simplest approach if RAM allows. No proxy, no swap latency.

### About what people actually use with the open-source agent CLIs

(Researched community sentiment via Brave Search — direct Reddit was blocked.)

- **Pi**: Ollama is featured first in pi-mono's own docs; `pi-llama-cpp` extension exists for power users on llama-server. LM Studio is the GUI option. (Sources: [pi-mono/coding-agent/docs/models.md](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/models.md), [Ollama Pi integration](https://docs.ollama.com/integrations/pi))
- **OpenCode**: "Ollama is the most common choice for solo coding setups."
- **Aider**: tutorials default to Ollama; works directly with its OpenAI-compat API.
- **Goose**: Ollama is the documented air-gapped path; "Goose plus Ollama" is the recommended local stack.
- **Migration pattern**: developers start on Ollama for setup ease, graduate to raw `llama.cpp` only when they're running 500+ inferences/day and the 15–25% speed gap matters. For BoBe's intermittent background workers, that gap is irrelevant.
- **`pi_agent_rust`**: Rust port by Jeff Emanuel ([repo](https://github.com/Dicklesworthstone/pi_agent_rust), 873 stars, [crates.io](https://crates.io/crates/pi_agent_rust), v0.1.7). One contributor, fewer eyeballs than upstream — too thin a bus factor to base BoBe on.
- **Goose Rust crates**: workspace at `aaif-goose/goose` has `crates/goose`, `goose-sdk`, `goose-mcp`, `goose-acp`, etc. — but **none are published to crates.io**. The name `goose` is taken on crates.io by an unrelated load-testing tool. Using Goose means a `git` dependency or vendoring.

### About networking constraints to plan around

- **Reddit is blocked** from Anthropic's WebSearch and WebFetch tools (per their crawler policy). `web.archive.org` is also blocked. Most libreddit/redlib mirrors are dead since Reddit's 2023 API kill. Brave Search works to surface Reddit snippets via search results. HN Algolia (`hn.algolia.com/api/v1/search`) returns JSON directly without auth.

---

## Architectural decisions

### What we picked

| Decision | Rationale |
|---|---|
| **Stay on `github-copilot-sdk`** as the only agent loop | Mature SDK, MS-maintained, agent loop (skills, hooks, autopilot, MCP, tool dispatch) already integrated and tested in BoBe. Switching loops means deleting and re-implementing infrastructure we just built. |
| **Bundle the CLI binary** via `embedded-cli` feature | No "is Copilot CLI installed?" failure mode. Wizard step 2 simplified. PATH-vs-GUI-app discrepancies (Homebrew vs sandboxed app) eliminated. ~60 MB binary growth, acceptable. |
| **Two engine modes**: `copilot_cloud` (default) and `local` | Cleanest split: one user picks, daemon configures CLI accordingly. Auth, model, base-URL all derive from this single discriminator. |
| **Ollama as local runtime** | Community standard for Pi/OpenCode/Aider/Goose. Native multi-model + smart scheduler + model registry (`ollama pull`). Pulled muscle memory matches the ecosystem. |
| **Download Ollama on demand** (don't bundle) | Bundling complicates code-signing/notarization (third-party binary inside our bundle), forces lockstep version pinning, inflates every BoBe download by ~250 MB even for cloud-only users. Pre-pivot pattern from `main` already shows how to do this cleanly. |
| **Detect existing Ollama on `:11434`** before spawning ours | Power users who already run Ollama see no second copy. Their `~/.ollama/models` cache is reused — models they previously pulled don't re-download. |
| **Two Copilot CLI subprocesses in local mode** (text-client + vision-client) | BYOK fixes `COPILOT_MODEL` at spawn. To use different text and vision models we need different processes. Memory cost (~430 MB Node RAM total) is acceptable on 16+ GB Macs. |
| **Default models**: Qwen 2.5 7B Instruct + Qwen 2.5-VL 7B | Both ~5 GB at Q4_K_M. Total local-mode footprint ~10 GB models + ~10 GB resident. Strong text quality from the dedicated text model; strong vision from the dedicated VL. |
| **`Attachment::Blob` for images** (in-memory base64) | Already what BoBe does. No disk roundtrip needed. SDK passes it as multimodal user-message content; vision-capable models on either side handle it. |
| **BoBe never proxies inference calls** | Daemon sets env vars at Client construction, period. SDK + spawned CLI handle every actual call to GitHub or Ollama. Less code, fewer failure modes, no in-process HTTP shim. |
| **Restart-required for engine + provider fields** | Spawned CLI captures `COPILOT_PROVIDER_*` env at boot. Standard pattern via existing `STATIC_FIELDS` + `RestartRequiredBanner`. |

### What we rejected and why

- **`pi_agent_rust`** — single contributor, ecosystem risk. The crate is sound but BoBe shipping a daemon to users needs more bus-factor than one person.
- **`goose`** — not on crates.io (name collision with load-testing tool). git deps degrade `cargo audit` and CI semantics. Architecturally a strong fit (Rust crate, MCP-native, AAIF-governed since 2025), but the distribution gap is real today.
- **`opencode-sdk-rs`** — HTTP+SSE topology means spawning `opencode serve` as a sidecar. Works but doubles the architectural surface vs reusing our existing CLI-subprocess pattern.
- **`claude-agent-sdk-rs`** + LiteLLM redirect — vendor-locked SDK abused to drive non-Anthropic models. Reddit reports it works but is fragile (undocumented `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, beta-header rejection). Not a user-facing path.
- **In-process FFI to llama.cpp** (via `llama-cpp-2`) — saves the loopback HTTP hop (~50 µs) which is irrelevant vs seconds of inference. Costs: cmake + Metal toolchain in BoBe build, macOS notarization complexity for linked native code, in-process model crashes take BoBe down. Net negative.
- **Abandoning the Copilot agent loop** for direct llama.cpp FFI — would mean re-implementing skills, hooks, autopilot, MCP server lifecycle, tool dispatch, JSON repair, multi-turn iteration. ~3K+ lines of Rust to break even with what we have today.
- **`llama-server` + `llama-swap` bundled** — we'd be reinventing Ollama (model download + GGUF cache + multi-model scheduler) for marginal speed gain (~15–25%) on a workload (background workers) where it's irrelevant.
- **LlamaEdge bundled** — smallest at 30 MB but no native multi-model, smaller community, would still need `llama-swap` on top. The 60 MB savings vs llama.cpp is dwarfed by 10 GB of model weights.
- **Bundling Ollama in the .app** — even at the corrected ~200 MB macOS size, third-party binary embedding complicates code-signing/notarization and forces version lockstep with our own releases. Downloading-on-demand matches the pre-pivot pattern that worked.
- **Single VL model handling everything** — small VL models (7B) lose text quality vs dedicated text peers. The gap closes at 32B+ but our default scale is 7B; user-perceptible regressions outweigh the operational simplicity.
- **Disk-backed image attachments** — `Attachment::Blob` (in-memory base64) is already optimal. Disk roundtrip would be a regression. The vision-gap problem is model capability (text-only models can't "see" images), not transport.
- **In-process HTTP shim with FFI** (the loopback-with-FFI hybrid I proposed at one point) — strictly more complex than running `llama-server` (or Ollama) as an external subprocess. The TCP between CLI and the inference backend is unavoidable; trying to "save" it inside our process buys nothing.

---

## Final architecture

```
┌──────────────────────────────────────────────────────────────────┐
│ BoBe daemon (Rust binary, ~120 MB after stripping)               │
│                                                                  │
│   github-copilot-sdk (with embedded-cli feature, Phase 0)        │
│   ├─ extracts copilot CLI to ~/.cache/github-copilot-sdk-{ver}/  │
│   └─ spawns 1 client (cloud) or 2 clients (local: text+vision)   │
│                                                                  │
│   binary_manager (resurrected from main, simplified)             │
│   ├─ downloads Ollama to ~/.bobe/runtimes/ollama if absent       │
│   ├─ SHA-256 verified against official release SHA256SUMS.txt    │
│   └─ progress streamed via watch::Sender<DownloadProgress>       │
│                                                                  │
│   ollama_manager (resurrected from main)                         │
│   ├─ probes localhost:11434 → uses user's Ollama if present      │
│   ├─ otherwise spawns `ollama serve` and supervises              │
│   └─ orchestrates `ollama pull qwen2.5:7b-instruct` + `:vl-7b`   │
│      with progress events                                        │
│                                                                  │
│   /local-runtime/install (POST) + /local-runtime/status (SSE)    │
│   wired into the wizard's local-card flow                        │
│                                                                  │
└──┬───────────────────────────────────────────────────────────────┘
   │ stdio JSON-RPC                          (env vars set ↑ at Client.start)
   ▼
┌─────────────────────────────────┐
│ Copilot CLI subprocesses        │  COPILOT_PROVIDER_BASE_URL =
│                                 │      http://127.0.0.1:11434/v1 (local)
│   Cloud mode: 1 client          │              ─OR─
│   Local mode: 2 clients         │      (unset for cloud — defaults to GitHub)
│     text-client COPILOT_MODEL=  │
│       qwen2.5:7b-instruct       │ ──────────┐
│     vision-client COPILOT_MODEL=│            ▼
│       qwen2.5-vl:7b             │   ┌────────────────────────┐
└──┬──────────────────────────────┘   │ Local: ollama serve    │
   │ HTTPS (cloud)         (local)    │ Native multi-model     │
   ▼                                  │ OLLAMA_MAX_LOADED=2    │
GitHub Copilot                        │ Models: ~/.ollama/     │
(no Ollama in this path)              └────────────────────────┘
                                      (NOT in BoBe's request path —
                                       SDK + CLI handle calls direct)
```

Worker-class routing in local mode:
- `Chat`, `Decide`, `Goals`, `Consolidate` → **text-client**
- `Vision` (capture pipeline) → **vision-client**

In cloud mode there's one client and all worker classes share it (current behavior).

---

## Phased plan

Each phase is bounded enough to land as a focused commit (or a small group). Phases beyond 0–1 haven't shipped yet.

### Phase 0 — Bundle Copilot CLI binary ✅ (committed `f47c7a3`)

- `BoBeService/Cargo.toml`: enable `features = ["embedded-cli"]` on `github-copilot-sdk`
- `BoBeService/.cargo/config.toml`: pin `COPILOT_CLI_VERSION = "1.0.44"` (latest stable)
- Welcome wizard step 2 simplified: drop `which copilot` probe, drop install-instructions branch
- 6 dead `setup.copilot.*` i18n keys pruned across 9 locales
- `ENGINE_PROVIDER_NOTES.md` (this doc) created

### Phase 1 — Engine + provider settings DTO (data plumbing) ✅ (committed `bc7dc8b`)

- `config.rs`: new `EngineConfig` sub-struct with `engine`, `provider_base_url`, `provider_text_model`, `provider_vision_model`, `provider_offline`. Default `{engine: "copilot_cloud", provider_offline: true}` so cloud-only users see no behavior change.
- `config_manager`: 5 new dotted keys added to `STATIC_FIELDS` (restart-required); `fields.rs` parser arms + flat-key normalization.
- `api/handlers/settings.rs`: `SettingsResponse` + `SettingsUpdateRequest` extended; `get_settings` projects, `update_settings` collects via `collect_opt!`.
- `Models/SettingsTypes.swift`: mirror with snake_case `CodingKeys`.
- **Not yet activated**: env-var passthrough at Client construction. Daemon reads the fields but doesn't act on them. Lands in Phase 5b.

### Phase 2 — Wizard rework ✅ partial (cards committed `76f0e52`)

**Cards UI shipped.** Step 2 is now an `EngineChoiceStepView` with two cards:
- "Use GitHub Copilot" — needs subscription
- "Run AI on this Mac" — needs ~32 GB RAM ideally

User selection persists via `EngineChoice.persist()` to `UserDefaults["bobe.engine_choice"]`. Daemon reads this on next start — wired in Phase 5.

**Auth check (cloud branch)** — *deferred to Phase 2b (#57)*. After cloud-card selection, daemon `GET /auth/status` (which calls SDK's `client.get_auth_status()`) returns `{is_authenticated, login, status_message}`. Wizard surfaces "Sign in" CTA via Terminal shellout if not authed; affirms "Signed in as @login" if authed.

**Local server detection (local branch)** — *deferred to Phase 5d (#61)*. After local-card selection, wizard transitions to `LocalSetupStepView` driven by Phase 5c SSE.

### Phase 5a — Resurrect `binary_manager` (#58)

Bring back `BoBeService/src/binary_manager/{mod, download, extract}.rs` from `main` (~460 lines total). Simplifications vs the pre-pivot version:
- Drop multi-arch matrix → macOS arm64 + x64 only
- Drop the `OllamaProvider` / `LlmProvider` glue → only `ensure_managed_ollama` survives
- Re-add `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "stream"] }` to `Cargo.toml` (was nuked during cleanup; needed for streaming downloads with progress)

Public API:
```rust
pub(crate) struct BinaryManager { /* data_dir, http_client */ }

impl BinaryManager {
    pub(crate) fn new(data_dir: &Path, http: Arc<reqwest::Client>) -> Self;

    /// Returns path; downloads to ~/.bobe/runtimes/ollama if absent or
    /// hash-mismatched. Idempotent — safe to call repeatedly.
    pub(crate) async fn ensure_managed_ollama(
        &self,
        progress_tx: &watch::Sender<DownloadProgress>,
    ) -> Result<PathBuf, AppError>;
}

pub(crate) struct DownloadProgress {
    pub(crate) current_bytes: u64,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) percent: Option<u8>,
}
```

Tests: download stub against a local file URL; SHA-256 mismatch returns `AppError::Integrity`.

### Phase 5b — Resurrect `ollama_manager` + runtime service + two-Client topology (#59)

Two pieces in one commit because they're tightly coupled.

**`copilot/ollama_manager.rs`** (~200 lines, restored from main with simplifications):
```rust
pub(crate) struct OllamaManager { /* http_client */ }

impl OllamaManager {
    /// Probes 127.0.0.1:11434/api/version. Returns true if user's Ollama
    /// is already running; we don't spawn ours in that case.
    pub(crate) async fn detect_running(&self) -> bool;

    /// Spawn `{binary_path} serve` as a managed subprocess. No-op if a
    /// daemon is already responding on :11434. Returns when the new
    /// daemon is healthy (polls /api/version).
    pub(crate) async fn ensure_daemon_running(
        &self,
        binary_path: Option<&Path>,
        auto_start: bool,
    ) -> Result<(), AppError>;

    /// Shells out to `ollama pull NAME`, parses progress lines from
    /// stderr, streams via progress_tx. Returns when complete.
    pub(crate) async fn pull_model(
        &self,
        name: &str,
        progress_tx: &watch::Sender<DownloadProgress>,
    ) -> Result<(), AppError>;

    /// `GET /api/tags` listing — used to skip pulls for models the
    /// user has already cached.
    pub(crate) async fn list_models(&self) -> Result<Vec<String>, AppError>;
}
```

**`services/ollama_runtime_service.rs`**: orchestrates the binary + manager:
```rust
pub(crate) struct OllamaRuntimeService { /* state */ }

impl OllamaRuntimeService {
    /// End-to-end first-launch flow for local mode:
    /// 1. Probe :11434 — if responding, skip download
    /// 2. binary_manager.ensure_managed_ollama (with progress_tx[0])
    /// 3. ollama_manager.ensure_daemon_running
    /// 4. ollama_manager.list_models — diff against required set
    /// 5. pull_model x N in parallel for missing ones (progress_tx[1], [2])
    pub(crate) async fn install(
        &self,
        text_model: &str,
        vision_model: &str,
        progress_txs: [watch::Sender<DownloadProgress>; 3],
    ) -> Result<(), AppError>;
}
```

**Two-Client topology in `copilot/client.rs` and `copilot/registry.rs`:**

```rust
pub(crate) struct ClientHandle { /* OnceCell, opts: ClientOptions */ }

impl ClientHandle {
    pub(crate) fn new(opts: ClientOptions) -> Arc<Self>;
    pub(crate) async fn ensure_started(&self) -> Result<Arc<Client>, AppError>;
}

// In WorkerRegistry:
//   cloud mode: client_text = client_vision = single ClientHandle
//   local mode: separate ClientHandle per role with different env vars
//
// Workers get the right client by class:
//   Chat/Decide/Goals/Consolidate → registry.client_text()
//   Vision                         → registry.client_vision()
```

The client_options builder reads `Config.engine` and assembles env vars accordingly:

```rust
fn client_options_for_role(cfg: &Config, role: ClientRole) -> ClientOptions {
    let mut opts = ClientOptions::default();
    if cfg.engine.engine == "local" {
        let base = cfg.engine.provider_base_url
            .as_deref()
            .unwrap_or("http://127.0.0.1:11434/v1");
        opts.env.push(("COPILOT_PROVIDER_BASE_URL".into(), base.into()));
        let model = match role {
            ClientRole::Text => &cfg.engine.provider_text_model,
            ClientRole::Vision => &cfg.engine.provider_vision_model,
        };
        if let Some(m) = model {
            opts.env.push(("COPILOT_MODEL".into(), m.into()));
        }
        if cfg.engine.provider_offline {
            opts.env.push(("COPILOT_OFFLINE".into(), "true".into()));
        }
    }
    opts
}
```

### Phase 5c — `/local-runtime/install` + `/local-runtime/status` SSE (#60)

New axum routes:
- `POST /local-runtime/install` — accepts `{ text_model, vision_model }` body, kicks off `OllamaRuntimeService::install`, returns 202 immediately
- `GET /local-runtime/status` (SSE) — streams `{ stage: "ollama" | "text-model" | "vision-model", percent, bytes_done, bytes_total, complete: bool, error?: string }` events
- `POST /local-runtime/cancel` — aborts an in-flight install

Daemon stores the in-flight task handle; concurrent installs return 409.

### Phase 5d — Wizard local-card download UI (#61)

After `EngineChoiceStepView` selection of "local", wizard advances to `LocalSetupStepView`:

1. Hardware check via `sysctl hw.memsize` (Swift's `Host.current().info()` or shellout). Warn if `<16 GB`; recommend `32 GB+`. Don't block.
2. Three stacked `BobeLinearProgressBar` instances, each driven by its `stage` field from the SSE stream:
   - Ollama runtime
   - Text model (Qwen 2.5 7B Instruct)
   - Vision model (Qwen 2.5-VL 7B)
3. "Cancel" button → `POST /local-runtime/cancel`, returns to engine choice
4. On all-three-complete → wizard PATCHes settings with `{engine: "local", provider_base_url: ..., provider_text_model: ..., provider_vision_model: ...}`, advances to permissions step

Local-card flow takes ~8–15 minutes on a fast connection (10 GB of pulls). UI must stay responsive; user can click away and come back.

### Phase 3 — Settings Engine pane (#52)

New top-level Settings category `Engine` (sidebar grows to 10 categories). Or fold into existing `Advanced` if 9 is the cap — decide during impl.

UI elements:
- Radio: Cloud / Local (engine field)
- (Local) Editable text fields: provider_base_url, provider_text_model, provider_vision_model
- (Local) Toggle: provider_offline
- (Cloud) "Sign in to GitHub Copilot" button shown only when `client.get_auth_status()` reports `is_authenticated: false`
- Restart-required banner extends shadow set with the 5 engine fields

### Phase 4 — Returning-user migration (#53)

Existing users have `bobe.onboarding_completed = true`; they won't see the new wizard. Two-line fix:
- Daemon defaults to `engine: "copilot_cloud"` on first read after upgrade (already true in Phase 1's defaults)
- Add a one-time non-blocking pill in the overlay if user opens Settings → Engine pane and `engine_choice` UserDefault is unset → "Welcome to local-AI mode! Set up here." Otherwise no nudge, behavior matches today.

### Phase 2b — Cloud-card auth-status check (#57)

After `EngineChoiceStepView` cloud selection, wizard advances to `AuthCheckStepView`:
- Daemon endpoint `GET /auth/status` calls SDK's `client.get_auth_status()`
- If `is_authenticated`: green check, "Signed in as @{login}" → continue
- If not: "Sign in" button shells out to `copilot login` in Terminal.app via NSWorkspace; "Retry check" re-probes; "Skip" advances anyway (user discovers auth issue on first chat)

---

## Default models + hardware floor

| Slot | Default model | Quantization | Disk size | Loaded RAM | Notes |
|---|---|---|---|---|---|
| Text | `qwen2.5:7b-instruct` | Q4_K_M | ~5 GB | ~4–5 GB | Strong text quality, great tool calling with `--jinja` |
| Vision | `qwen2.5-vl:7b` | Q4_K_M | ~5 GB | ~4–5 GB | Excellent image understanding; cleanly handles text-only requests too |

**First-launch download**: ~250 MB Ollama binary + ~10 GB models = ~10.25 GB total.

**RAM floor for local mode** (sum of resident processes):
- BoBe daemon: ~120 MB
- 2× Copilot CLI subprocesses: ~430 MB
- Ollama: ~50 MB (idle); model weights add ~10 GB when both loaded
- macOS overhead + user apps: ~4 GB
- **Total**: ~14.5 GB → realistically need **16 GB minimum, 32 GB recommended**.

Wizard surfaces a warning if `sysctl hw.memsize` returns < 16 GB. User can proceed but is informed.

---

## Open questions / risks

- **macOS notarization with bundled CLI**: the `embedded-cli` feature extracts the Copilot CLI binary at runtime to user cache; first-launch needs to pass codesign verification or carry appropriate entitlements (`com.apple.security.cs.allow-jit`, `disable-library-validation`). Verify on a fresh signed `just build`.
- **Two Copilot CLI subprocesses' RAM**: ~430 MB for the orchestrators alone. Acceptable on 16+ GB Macs; could be a tight squeeze with model weights loaded on 16 GB. Profile and consider single-client fallback for low-memory users.
- **Default model size**: 5 GB × 2 = 10 GB download. Some users on metered connections will resent this. Maybe surface "smaller models available — needs less disk" option behind a settings advanced toggle.
- **Qwen 2.5-VL 7B text quality vs Qwen 2.5 7B**: I assumed the gap is meaningful but couldn't find a clean head-to-head benchmark. If real-world testing shows the gap is smaller than expected, the two-Client architecture might be over-engineered. Watch for user feedback.
- **CLI version pinning policy**: `COPILOT_CLI_VERSION = "1.0.44"` is hand-set. Bumping is deliberate. Need a process / CI alert when upstream releases new versions; eventually wire to dependabot.
- **Cache cleanup on Copilot CLI upgrade**: when we bump `COPILOT_CLI_VERSION`, the old `~/.cache/github-copilot-sdk-{old-ver}/` dir lingers. One-time cleanup task on first run after upgrade.
- **Ollama auto-update**: bundled-into-`~/.bobe/runtimes/` Ollama doesn't auto-update; user's brew-installed Ollama would. We could surface "newer Ollama available" in Settings → Engine, prompt to re-run `binary_manager` with new version. Defer until users hit it.
- **Cancellation mid-pull**: if user cancels during `ollama pull`, partial blob is left in `~/.ollama/models`. Ollama tolerates this on retry but takes disk space. Background task to clean partial pulls? Defer.
- **`reqwest` adds back ~70 dependencies** to BoBe's tree. Manageable but not free. Alternative: use `ureq` (sync, smaller) with a `tokio::task::spawn_blocking` wrapper. Pick during Phase 5a.
- **No clear "uninstall local mode"** flow — if user picks local and regrets it, settings let them switch back to cloud, but the 10 GB of models stays in `~/.ollama/`. Surface a "Free disk space" button in Settings → Engine that calls `ollama rm qwen2.5:7b qwen2.5-vl:7b`?
- **Subscription gate detection** for cloud is impossible pre-auth. We can warn in step 2 copy ("requires Copilot subscription"), but first call still fails ungracefully if the user has no subscription. Document the failure mode.

---

## Sources

### Copilot CLI / SDK
- [GitHub Changelog: Copilot CLI BYOK + local models (2026-04-07)](https://github.blog/changelog/2026-04-07-copilot-cli-now-supports-byok-and-local-models/)
- [GitHub Docs: BYOK env vars](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-byok-models)
- [Ollama Copilot CLI integration](https://docs.ollama.com/integrations/copilot-cli)
- [github/copilot-sdk monorepo (TS, Python, Go, .NET, Java, Rust)](https://github.com/github/copilot-sdk)
- [Issue #248 — bundling discussion](https://github.com/github/copilot-sdk/issues/248)
- [Steve Sanderson confirmation comment](https://github.com/github/copilot-sdk/issues/248#issuecomment-3813716744)
- [`rust/build.rs` — bundling impl](https://github.com/github/copilot-sdk/blob/main/rust/build.rs)
- [`rust/src/embeddedcli.rs` — runtime extract](https://github.com/github/copilot-sdk/blob/main/rust/src/embeddedcli.rs)
- [`rust/src/resolve.rs` — binary resolve order](https://github.com/github/copilot-sdk/blob/main/rust/src/resolve.rs)
- [Copilot CLI releases](https://github.com/github/copilot-cli/releases)
- [Issue #2531 — local AI model support history](https://github.com/github/copilot-cli/issues/2531)
- [Mainbranch: BYOK with Ollama + Gemma](https://mainbranch.dev/articles/copilot-cli-byok-ollama/)
- [Laminar: instrumenting Claude Agent SDK with a tiny Rust proxy](https://laminar.sh/blog/2025-12-03-claude-agent-sdk-instrumentation) — useful pattern for subprocess-based SDK observability

### Local runtimes — Ollama, llama.cpp, LlamaEdge
- [Ollama macOS docs](https://docs.ollama.com/macos)
- [Ollama FAQ — concurrency + multi-model](https://docs.ollama.com/faq)
- [Ollama new model scheduling](https://ollama.com/blog/new-model-scheduling)
- [llama.cpp tools/server README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [llama.cpp function-calling docs](https://github.com/ggml-org/llama.cpp/blob/master/docs/function-calling.md)
- [llama.cpp router mode (HF blog)](https://huggingface.co/blog/ggml-org/model-management-in-llamacpp)
- [llama-swap GitHub](https://github.com/mostlygeek/llama-swap)
- [llama-swap setup guide 2026](https://modelslab.com/blog/api/hot-swap-local-llms-instantly-llama-swap-setup-guide-2026)
- [LlamaEdge GitHub](https://github.com/LlamaEdge/LlamaEdge)
- [LlamaEdge vs Ollama](https://llamaedge.com/docs/llamaedge_vs_ollama/)
- [Anthropic Messages API in llama.cpp](https://huggingface.co/blog/ggml-org/anthropic-messages-api-in-llamacpp)
- [Unsloth: llama-server + OpenAI endpoint deployment](https://unsloth.ai/docs/basics/inference-and-deployment/llama-server-and-openai-endpoint)

### Models — Qwen 2.5/3, vision-language landscape
- [Qwen 2.5-VL 7B (Qwen blog)](https://qwen.ai/blog?id=qwen2.5-vl)
- [Qwen 2.5: foundation models](https://qwenlm.github.io/blog/qwen2.5/)
- [Qwen3 technical report (PDF)](https://arxiv.org/pdf/2505.09388)
- [Qwen3-VL usage guide (vLLM)](https://docs.vllm.ai/projects/recipes/en/latest/Qwen/Qwen3-VL.html)
- [Top open-source vision-language models in 2026 (BentoML)](https://www.bentoml.com/blog/multimodal-aiao-guide-to-open-source-vision-language-models)
- [Qwen 2.5-VL vs Llama 3.2 Vision](https://www.labellerr.com/blog/qwen-2-5-vl-vs-llama-3-2/)
- [Best LLMs for OpenCode tested locally (Glukhov)](https://www.glukhov.org/ai-devtools/opencode/llms-comparison/)

### Agent CLI ecosystem (Pi, OpenCode, Aider, Goose)
- [pi-mono GitHub](https://github.com/badlogic/pi-mono)
- [pi-mono coding-agent docs](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/)
- [pi-mono local LLM models doc](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/models.md)
- [pi_agent_rust (Rust port)](https://github.com/Dicklesworthstone/pi_agent_rust)
- [pi_agent_rust on crates.io](https://crates.io/crates/pi_agent_rust)
- [Mario Zechner: building pi-coding-agent](https://mariozechner.at/posts/2025-11-30-pi-coding-agent/)
- [OpenCode docs](https://opencode.ai/docs/)
- [opencode-sdk-rs](https://docs.rs/opencode-sdk-rs/latest/opencode_sdk_rs/)
- [block/goose (now under AAIF)](https://github.com/block/goose)
- [Goose AGENTS.md](https://github.com/block/goose/blob/main/AGENTS.md)
- [Aider chat](https://aider.chat)

### Comparison and benchmark articles
- [grigio.org: OpenCode vs Pi which to use](https://grigio.org/opencode-vs-pi-which-ai-coding-agent-should-you-use/)
- [grigio.org: Local Harness Benchmark — Pi vs OpenCode](https://grigio.org/local-harness-benchmark-pi-coding-agent-vs-opencode/)
- [grigio.org: OpenCode vs Pi local LLM benchmarks](https://grigio.org/opencode-vs-pi-local-llm-benchmark-results/)
- [thoughts.jock.pl: Which AI coding harness actually works without you?](https://thoughts.jock.pl/p/ai-coding-harness-agents-2026)
- [Tembo: 2026 guide to coding CLI tools — 15 agents compared](https://www.tembo.io/blog/coding-cli-tools-comparison)
- [Pinggy: Top 5 CLI coding agents 2026](https://pinggy.io/blog/top_cli_based_ai_coding_agents/)
- [Pinggy: Best self-hosted LLMs for coding 2026](https://pinggy.io/blog/best_open_source_self_hosted_llms_for_coding/)
- [The Register: How to roll your own local AI coding agents](https://www.theregister.com/2026/05/02/local_ai_coding_agents/)
- [Tildes: Is it worthwhile to run local LLMs for coding today?](https://tildes.net/~comp/1t2j/is_it_worthwhile_to_run_local_llms_for_coding_today)
- [bitdoze: OpenCode vs Pi Agent](https://www.bitdoze.com/opencode-vs-pi-agent/)
- [Openxcell: llama.cpp vs Ollama 2026](https://www.openxcell.com/blog/llama-cpp-vs-ollama/)
- [Red Hat: vLLM vs llama.cpp inference engines](https://developers.redhat.com/articles/2025/09/30/vllm-or-llamacpp-choosing-right-llm-inference-engine-your-use-case)
- [DEV Community: Why Ollama overhead exists](https://dev.to/plasmon_imp/ollama-lm-studio-and-gpt4all-are-all-just-llamacpp-heres-why-performance-still-differs-59h5)
- [Mastering multi-model stacks with llama-swap](https://dasroot.net/posts/2026/05/mastering-multi-model-stacks-llama-swap/)

### Rust ML ecosystem (for context — we're not using these)
- [llama-cpp-2 on crates.io](https://crates.io/crates/llama-cpp-2)
- [llama_cpp safe high-level bindings](https://docs.rs/llama_cpp)
- [llama-cpp-4 (newer fork)](https://crates.io/crates/llama-cpp-4)
- [mistral.rs (Candle-based, multimodal)](https://github.com/EricLBuehler/mistral.rs)
- [Hugging Face Candle](https://github.com/huggingface/candle)
- [ushi — production llama.cpp inference server in Rust](https://lib.rs/crates/ushi)

### Process notes
- [Anthropic crawler policy (why Reddit can't be fetched)](https://support.anthropic.com/en/articles/8896518)
- [HN Algolia API — for adjacent dev opinions](https://hn.algolia.com/)

---

## Cross-references

- Plan-of-record: this file (`ENGINE_PROVIDER_NOTES.md`)
- Earlier SwiftUI catch-up plan: `SWIFTUI_CATCHUP_PLAN.md` (now-completed work that landed before the engine work)
- Memory: `/Users/john/.claude/projects/-Users-john-Repos-bobrust/memory/project_copilot_workers_initiative.md`
- Daemon source layout: `BoBeService/src/{copilot,api,config,config_manager,services}/`
- Swift source layout: `BoBeMacUI/BoBe/{Models,Services,Stores,Views/Setup,Features/Settings}/`

---

*Generated 2026-05-10. Single source of truth for engine + provider architecture decisions on this branch.*
