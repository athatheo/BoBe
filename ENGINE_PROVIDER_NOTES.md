# Engine + Provider Plan — discoveries, decisions, and phased rollout

> **File:** `ENGINE_PROVIDER_NOTES.md` (repo root)
> **Branch:** `feat/copilot-sdk-pivot`
> **Last updated:** 2026-05-10
> **Status of work:** **All planned phases shipped.** Foundation + endpoints + Phase 5a-5d + Phase 2b + Phase 3 + Phase 4 are landed locally on `feat/copilot-sdk-pivot`. Smoke test against a live daemon (#28) and live MCP runtime state (#31) remain. See [What's been shipped](#whats-been-shipped) for the full commit list.

---

## Quick links

- [TL;DR](#tldr)
- [What's been shipped](#whats-been-shipped)
- [Layered mental model — agent loops vs SDKs vs runtimes](#layered-mental-model)
- [Research findings](#research-findings)
- [Architectural decisions](#architectural-decisions)
- [What we rejected and why](#what-we-rejected-and-why)
- [Final architecture](#final-architecture)
- [Phased plan (remaining)](#phased-plan-remaining)
- [Default models + hardware floor](#default-models--hardware-floor)
- [Open questions](#open-questions)
- [How to research things in this space](#how-to-research-things-in-this-space)
- [Sources](#sources)

---

## TL;DR

1. **One agent loop**: stay on `github-copilot-sdk` (Rust). It's bundled into BoBe via the `bundled-cli` feature so users never have to install Copilot CLI separately. No second SDK.
2. **Two engine modes**: `copilot_cloud` (default) and `local` (Ollama). User picks at first-launch wizard; configurable later in Settings → Engine. **Hot-swap** — `ConfigManager` notifies `WorkerRegistry::reload()` on engine changes; the next worker access re-spawns the CLI against the new config. No daemon restart.
3. **One shared Copilot CLI subprocess in either mode**. Per-session `model` + `provider` overrides via `SessionConfig::with_model` / `with_provider` (verified in `github-copilot-sdk` 1.0.6).
4. **Per-class model slots**: chat (user-facing dialogue), batch (goals / decide / consolidate autopilot jobs), vision (capture pipeline). Five worker classes, three model slots — Decide / Goals / Consolidate share the batch slot.
5. **Local runtime is downloaded on demand, not bundled**: Ollama (~200 MB) and the Qwen models (~10 GB total) are fetched the first time the user picks local mode. Existing Ollama on `:11434` is detected and reused.
6. **BoBe never proxies inference**: daemon configures the SDK's `SessionConfig` per worker. SDK + spawned CLI handle every actual call. No in-process HTTP shim, no FFI, no proxy.
7. **Images stay in-memory** via `Attachment::Blob` (base64 in JSON-RPC). No disk roundtrip — already optimal.

---

## What's been shipped

All commits on `feat/copilot-sdk-pivot`. The engine-pivot work is everything from `f47c7a3` forward:

```
9c9b931 engine(phase-3)         — Settings → Engine pane (radio + model dropdowns)
18961ad engine(phase-5d+2b)     — wizard local-setup + cloud-auth steps
6154688 engine(phase-5b+5c)     — ollama_manager + install service + SSE endpoints
8fc1bad engine(phase-5a)        — resurrect binary_manager for Ollama download
2e2a96c engine(endpoints)       — GET /auth/status + GET /models (cloud or local)
f08b34c engine(foundation)      — hot-swap engine + per-session BYOK + chat/batch/vision split
415e3b7 engine(plan)            — expand discoveries + clarify two-CLI != two-models
cb65116 engine(plan)            — comprehensive plan-of-record (this doc, prior version)
bc7dc8b engine(phase-1)         — engine + provider settings DTO (data plumbing)
43278f8 engine(plan)            — early doc revision: download-on-demand pivot
76f0e52 engine(wizard)          — wizard step 2 = cloud-vs-local cards
f47c7a3 engine(phase-0)         — bundle Copilot CLI via embedded-cli feature
1ee9164 fidelity(swift-overlay) — /status sync, soft warnings, tool badge, busy 409
686dbf9 fidelity(swift-mcp)     — hide stub connected/tool_count
0661443 fidelity(swift-settings) — persist_failed + saved-toast + DB-degraded
cc6e8d2 fidelity(daemon)        — vision breaker SSE + drop dead by-name routes
631a51f swiftui(i18n)           — purge dead namespaces, add wizard + section keys
c11f501 swiftui(welcome)        — first-launch wizard (4 steps)
22076cd swiftui(settings)       — sidebar trim + locale rerender + 4-card overview
a9c4a03 swiftui(privacy)        — split off PrivacyPanel, drop GoalWorkerPanel
2997967 swiftui(memory)         — single-doc MemoriesEditor over GET/PUT /memory
0ae0a1a swiftui(goals)          — rich-section GoalsEditor with archived CRUD
74c1a49 swiftui(advanced)       — rebuild AdvancedPanel + RestartRequiredBanner
62c38e1 swiftui(behavior)       — rebuild BehaviorPanel for 9-field DTO
```

### Phase 0 — Bundle Copilot CLI

`BoBeService/Cargo.toml` pins `github-copilot-sdk 1.0.6` with `bundled-cli`. The SDK embeds a verified platform archive, extracts lazily, and revalidates cached installations so partial or quarantined binaries are repaired automatically.

### Phase 1 — Engine + provider settings DTO

`config.rs`: new `EngineConfig { engine, provider_base_url, provider_text_model, provider_vision_model, provider_offline }`. Defaults `{engine: "copilot_cloud", provider_offline: true}`. Wired into `Config`. `config_manager`: 5 keys added to `STATIC_FIELDS` (restart-required at the time); `fields.rs` parser arms + flat-key normalization. `api/handlers/settings.rs`: `SettingsResponse` + `SettingsUpdateRequest` extended; `get_settings` projects, `update_settings` collects via `collect_opt!`. Swift `Models/SettingsTypes.swift` mirrored. *(Both the schema and the restart-required classification were revised in the foundation commit — see below.)*

### Foundation — schema rev + hot-swap + per-session BYOK

Verified in SDK 1.0.6: create and resume configs both accept model/provider overrides, allowing one shared CLI to host all five per-class sessions.

Schema rev: `provider_text_model` → `provider_chat_model` + `provider_batch_model`. Vision keeps its own slot. Three model knobs map to five worker classes:
- **Chat** → `provider_chat_model`
- **Vision** → `provider_vision_model`
- **Goals / Decide / Consolidate** → `provider_batch_model`

Hot-swap mechanism: all `engine.*` keys live in `HOT_SWAP_FIELDS`. Provider/engine changes hard-reload and forget incompatible sessions; model-only changes disconnect workers but preserve IDs, then resume with the new model.

`session_extras_for_class(cfg, class)` resolves `(model, provider)` per worker class from the live `EngineConfig` snapshot at session-create time. Cloud mode sets only `model`; local mode sets both.

### Endpoints — `/auth/status` + `/models`

Two sideband endpoints the wizard + Settings need:
- `GET /auth/status` wraps `Client::get_auth_status()`. Returns `{is_authenticated, auth_type, host, login, status_message}`. The bundled CLI shares auth state with the user's existing `gh` / `copilot` login — no separate BoBe sign-in.
- `GET /models?engine=…` lists models. Cloud → `Client::list_models()` (subscription-gated, includes vision flag + context window). Local → Ollama `/api/tags` (Ollama doesn't report context window). Engine override lets the wizard preview before flipping.

`reqwest` added as a top-level dep (rustls-tls + stream + json). `AppError::ServiceUnavailable` (HTTP 503) mapped for the local "Ollama not running" path.

### Phase 5a — `binary_manager` resurrected

Lifts the pre-pivot module structure (`mod` / `download` / `extract`) for the Ollama runtime: stat-only `find_managed_ollama`, idempotent `ensure_managed_ollama` with `watch::Sender<DownloadProgress>` events, gzip+tar extraction with path-traversal guard, `--version` validate. Adds `flate2` + `tar` deps.

### Phase 5b — `ollama_manager` + `OllamaInstallService`

`ollama_manager.rs`: thin wrapper around the user's (or our) Ollama daemon. Detection-first: `health_check` probes `:11434`, returning true if anyone is already running one. Spawns a managed `ollama serve` only when nobody is — and only SIGTERMs daemons we spawned (`Mutex<Option<Child>>` tracks ownership). `pull_model` streams Ollama's NDJSON pull events into a `watch::Sender<PullProgress>`, supports cancellation between chunks.

`services/ollama_install_service.rs` orchestrates the wizard's local-mode flow: ensure runtime → pull each model. Skips already-installed models, skips the batch pull when the same as chat. Single `watch::Sender<InstallSnapshot>` the SSE endpoint subscribes to.

### Phase 5c — `/local-runtime/{install, status, cancel}` endpoints

- `POST /local-runtime/install` body `{chat_model, batch_model, vision_model}`. Returns 202 + starts the task; 409 if one is in flight.
- `GET /local-runtime/status` (SSE) streams `InstallSnapshot` DTOs on every state change.
- `POST /local-runtime/cancel` flips a `watch` to true; the next chunk read in `pull_model` returns `Conflict("canceled")`.

`AppState` gains an `Arc<OllamaInstallService>` instantiated at bootstrap. Idle until the wizard fires a request — no background work for cloud-mode users.

### Phase 5d + 2b — wizard local-setup + cloud-auth

Restructures the linear 4-step wizard into a 6-step branched state machine. After engine choice, cloud users land on `CloudAuthStepView`; local users land on `LocalSetupStepView`. Both branches converge at permissions → done.

`LocalSetupStepView` auto-fires `POST /local-runtime/install` on appear with default models (`qwen2.5:7b-instruct` × 2 + `qwen2.5vl:7b`), subscribes to the SSE channel, binds three progress bars. RAM hint via `sysctlbyname("hw.memsize")`. Cancel button.

`CloudAuthStepView` hits `/auth/status`. Shows "@login signed in" if the bundled CLI sees existing auth; otherwise offers a "Sign in via Terminal" button that runs `osascript … gh auth login --scopes copilot`. Retry / Skip available throughout.

`DoneStepView` PATCHes `/settings` with the engine choice + per-class model defaults. The hot-swap listener catches the change and rebuilds the worker registry — launching the overlay lands on the chosen engine without a daemon restart.

Drops the `bobe.engine_choice` UserDefault — `Config.engine` is the authoritative source.

### Phase 3 — Settings → Engine pane

New `EnginePanel.swift` slotted into the Settings sidebar's integrations group. Exposes every engine field:
- **Mode** (radio): cloud / local. Hot-swap; flipping fires PATCH /settings; daemon's listener calls `WorkerRegistry::reload()`.
- **GitHub sign-in** (cloud only): pings `/auth/status`, shows "@login" or a "Sign in via Terminal" button.
- **Local server** (local only): editable `provider_base_url`, defaults to `http://127.0.0.1:11434/v1`.
- **Models** (always): three dropdowns (chat / batch / vision) populated from `/models?engine=…`. Vision dropdown filters to vision-capable models. "Use default" sentinel sets the field to nil.
- **Strict offline** (local only): toggles `provider_offline`.

Debounced save reuses the BehaviorPanel/AdvancedPanel convention. Engine-mode flip skips the debounce so the registry rebuilds promptly.

### Phase 4 — Returning-user migration

Zero-effort: returning users on upgrade get the default `engine: "copilot_cloud"` from `EngineConfig::default`. Hot-swap means they can switch via Settings → Engine without any restart prompt. No code change needed.

### Wizard cards (Phase 2a)

`WelcomeWizardSteps.swift`: replaced the affirmation-only step 2 with `EngineChoiceStepView` — two-card UI (cloud / local), each with a plain-language subtitle. User selection persists via new `EngineChoice` enum to `UserDefaults["bobe.engine_choice"]`. Continue button gated until a choice is made. Welcome step body cleaned: dropped "local-first" framing (misleading once user picks cloud) and the `~/.bobe/memory.md` path mention.

### Earlier in the session (referenced for context)

- **SwiftUI catch-up** (8 commits): rebuilt every broken Settings panel for the post-pivot daemon API — Behavior (9-field DTO), Advanced (3 fields), Goals (rich-section, full CRUD, archived in picker), Memories (single-doc CodeEditor + byte gauge), Privacy (single memory reset), SettingsWindow trim, first-launch welcome wizard, i18n purge across 9 locales. Ended with a green `swift build`.
- **E2E fidelity pass** (4 commits): vision-breaker SSE event from daemon (now surfaced as soft warning in overlay), `persist_failed` + saved-toast + degraded-DB warning in Settings, MCP panel hides stub `connected`/`tool_count` until task #31, `/status` sync on SSE reconnect, `Attachment::File`-vs-`Blob` decision (Blob stays — current code is optimal), 409 busy-state distinguished from fatal errors, conversation-ending notice, tool execution wrench badge.

---

## Layered mental model

A persistent confusion in this space — and one we tripped on early — is conflating three architectural layers. Naming them explicitly is what unblocked the Ollama-vs-Pi-vs-Goose decision.

```
Layer 3: SDK              (programmatic embed surface)
                          github-copilot-sdk, claude-agent-sdk-rs,
                          opencode-sdk-rs, pi-agent-core
                                  │ wraps / drives
                                  ▼
Layer 2: Agent loop       (the orchestrator that runs tool calls)
                          Copilot CLI, Claude Code, pi, opencode,
                          aider, goose, cline, continue
                                  │ inference HTTP calls to
                                  ▼
Layer 1: Model runtime    (the OpenAI-compat HTTP server)
                          Ollama, llama-server, LM Studio, vLLM,
                          Foundry Local, LlamaEdge
                                  │ loads
                                  ▼
                          GGUF / safetensors model weights
```

BoBe operates at all three:
- **Layer 3** = `github-copilot-sdk` Rust crate (chosen, bundled, in BoBe's `Cargo.toml`)
- **Layer 2** = Copilot CLI subprocess (extracted from the SDK's embedded archive)
- **Layer 1** = either GitHub's cloud (cloud mode) or Ollama (local mode)

Layer 2 is "the agent loop" — handles tool dispatch, multi-turn iteration, system prompt assembly, retry, JSON repair, autopilot mode, hooks, MCP server lifecycle, skills system. This is the layer we'd have to *re-implement* if we abandoned the Copilot agent loop in favor of direct Layer-1 calls. ~3K lines of work to break even with what we have today.

Layer 1 is just inference. Everyone (Ollama, llama-server, vLLM, etc.) speaks the OpenAI Chat Completions API as a common shape, which is why BYOK works at all.

The decision to stay on `github-copilot-sdk` is a Layer-2 + Layer-3 decision (we keep their agent loop and SDK). The Ollama choice is a Layer-1 decision (which inference server). They're orthogonal.

---

## Research findings

### Bundling and the `github-copilot-sdk` Rust crate

**The Rust SDK has a `bundled-cli` Cargo feature that bundles the Copilot CLI binary at build time.** The mechanism:

1. `Cargo.toml` enables `features = ["bundled-cli"]`.
2. The SDK build embeds the supported platform archive and its integrity metadata.
3. Runtime resolution checks `COPILOT_CLI_PATH`, then the bundled extraction.
4. Cached binaries receive an integrity re-check and are re-extracted when partial or corrupt.

At runtime, `install_bundled_cli()` exposes the verified extracted path for login and diagnostics.

Targets supported: macos-arm64, macos-x64, linux-x64, linux-arm64, windows-x64, windows-arm64.

BoBe currently pins Rust SDK 1.0.6.

### BYOK env vars in Copilot CLI

Shipped 2026-04-07 ([changelog](https://github.blog/changelog/2026-04-07-copilot-cli-now-supports-byok-and-local-models/), [docs](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-byok-models)):

| Env var | Purpose |
|---|---|
| `COPILOT_PROVIDER_BASE_URL` | Inference endpoint, e.g. `http://127.0.0.1:11434/v1` for Ollama |
| `COPILOT_MODEL` | Model name; for Ollama matches `ollama list` output |
| `COPILOT_PROVIDER_TYPE` | `openai` (default), `azure`, `anthropic` |
| `COPILOT_PROVIDER_API_KEY` | Auth token for cloud providers; **do not set for local Ollama** (empty key triggers failures) |
| `COPILOT_PROVIDER_WIRE_API` | `chat` (default) or `responses` (newer Ollama-style) |
| `COPILOT_OFFLINE` | `true` disables all GitHub-bound telemetry/metadata calls |
| `COPILOT_PROVIDER_MAX_PROMPT_TOKENS` | Override prompt token limit |
| `COPILOT_PROVIDER_MAX_OUTPUT_TOKENS` | Override output token limit |

**Constraints:**
- Endpoint must be `http://` or `https://`. **No UDS, no named pipes, no shared memory.** Confirmed from the docs and from the absence of any `socket_path` / `transport` options in the BYOK config.
- Models must support tool calling + streaming. Copilot CLI errors out otherwise.
- Recommended ≥64 K context window (system prompt is ~21 K tokens before user input).
- **`COPILOT_MODEL` is fixed at process spawn.** No mid-process model swap. This is the constraint that drives the two-Client topology in local mode.

### Auth status surface

`Client::get_auth_status() → GetAuthStatusResponse { is_authenticated: bool, auth_type: Option<String>, host: Option<String>, login: Option<String>, status_message: Option<String> }`.

`auth_type` values seen: `"user"`, `"env"`, `"gh-cli"`, `"hmac"`, `"api-key"`, `"token"`. We can distinguish "user already signed in via gh CLI" from "BoBe should prompt for sign-in".

`ClientOptions::use_logged_in_user` defaults to `true` when no `github_token` is provided — meaning if the user signed in via `copilot login` (or `gh auth login`) before launching BoBe, they're already good. The CLI also has a `--no-auto-login` flag if we ever need to suppress the auto-resolve.

### Image attachment surface

`Attachment` enum has four variants:

```rust
pub enum Attachment {
    File { path: PathBuf, display_name: Option<String>, line_range: Option<AttachmentLineRange> },
    Directory { path: PathBuf, display_name: Option<String> },
    Selection { file_path: PathBuf, text: String, display_name: Option<String>, selection: AttachmentSelectionRange },
    Blob { data: String /* base64 */, mime_type: String, display_name: Option<String> },
}
```

**BoBe's `VisionWorker` already uses `Blob`** — this is optimal. The user's earlier intuition that "writing image to disk + passing path" might be smarter was a non-issue: the Copilot CLI is a Node subprocess of BoBe, on the same machine, communicating over stdio JSON-RPC. The Blob's base64 bytes go through a kernel pipe (zero-copy on most kernels for the small-buffer case). Disk would be strictly slower and more code.

The vision-gap problem (text-only local models can't "see" images) is not a transport issue. It's a model capability issue, separate concern.

### Local-runtime alternatives — what we evaluated

The decision to bundle Copilot CLI into a single Rust SDK is now settled. The remaining choice is what Layer-1 runtime to recommend / bundle / spawn for local mode.

| Tool | Bundle size (macOS) | Multi-model | Hot-swap | Community default for | Notes |
|---|---|---|---|---|---|
| **Ollama** | ~100–300 MB | **native (`OLLAMA_MAX_LOADED_MODELS`)** | **native** | **Pi, OpenCode, Aider, Goose** all default to it | de facto standard; 6–30% perf overhead vs raw llama.cpp |
| llama-server (raw) | ~30 MB | router mode (one resident, 3–10 s swap) | needs `llama-swap` | secondary | reference impl, fastest |
| llama-server + llama-swap | ~40 MB | yes via groups | yes | niche | we'd be reinventing Ollama |
| LlamaEdge | ~30 MB | one model per WASM instance | needs `llama-swap` | niche | WebAssembly, less mature |
| `llama-cpp-2` (Rust FFI) | ~90 MB linked | one model per ctx | manual | niche | requires cmake + Metal toolchain in BoBe build |
| LM Studio | ~600 MB (Electron) | yes (multiplexer) | yes | GUI users | great GUI, server mode for API |
| vLLM | ~2 GB Python | yes | yes | NVIDIA-GPU production users | overkill for single-user Mac |

**The 4.6 GB Ollama figure that initially scared us off was wrong** — that was a Windows install with bundled CUDA + ROCm libraries, neither of which we need on macOS. The real macOS install is small enough to bundle.

We still chose to *download* rather than bundle (see [decisions](#architectural-decisions)) for code-signing simplicity and version-pinning flexibility.

### Hot-swapping and concurrent models

**Three patterns are mature in 2026:**

1. **`llama-server --models models.ini` (router mode)** — built into llama.cpp since mid-2024. Single endpoint, multi-model definitions, but only one resident at a time. Switch costs 3–10 seconds (full unload + reload).
2. **[`llama-swap`](https://github.com/mostlygeek/llama-swap)** — Go proxy in front of llama-server / Ollama / vLLM. Routes by `model` field. `groups` feature lets multiple models stay resident. TTL-based auto-unload. 3K+ stars, single binary, YAML config.
3. **Ollama's native multi-model + smart memory scheduler** — `OLLAMA_MAX_LOADED_MODELS=N` (default 3), auto-unloads to fit when a new model needs room, no manual orchestration.

For BoBe's text + vision use case, Ollama option 3 is cleanest — set `OLLAMA_MAX_LOADED_MODELS=2` and both models can stay warm.

### The Vision-Language quality tradeoff at small sizes

A real subtlety we worked through:

- **At 7B**: dedicated text models (e.g. Qwen 2.5 7B Instruct) outperform same-size VL variants (Qwen 2.5-VL 7B) on text-only benchmarks. The vision encoder + multimodal training takes parameters away from text capability.
- **At 32B+**: the gap closes. Qwen3-VL-235B-A22B-Instruct ranks #1 open model on lmarena.ai for *text*, despite being a VLM.
- **Why**: at scale, the model has enough parameters to learn both modalities well; at small sizes, every parameter "spent" on vision is one fewer for text.

**Implication for BoBe**: at the 7B class we ship, *running two models* (one text, one vision) gives meaningfully better text quality than *running one VL model for both*. But it's an *option*, not a *requirement*: the two-CLI architecture lets users configure both clients to point at the same model if they want a smaller setup.

Modern VL models (Qwen 2.5-VL, Qwen 3-VL) have special tokens cleanly separating vision from text inputs — passing a text-only request to a VL model doesn't engage the vision encoder. vLLM exposes `--limit-mm-per-prompt.image 0` for explicit text-only mode that frees the vision encoder's KV cache.

### Two CLIs ≠ Two models

To clarify a point that came up: the two-Copilot-CLI architecture in local mode is about **`COPILOT_MODEL` env-var flexibility**, not about forcing two distinct models loaded into memory.

- BYOK fixes `COPILOT_MODEL` at CLI process spawn. Can't change mid-process.
- BoBe wants per-worker-class control over which model gets called.
- Two CLIs let us assign one `COPILOT_MODEL` per role (text vs vision).
- **Both CLIs talk to the same Ollama instance.** Ollama routes by the `model` field of the request.
- If `provider_text_model == provider_vision_model`, both CLIs hit the same model — that's fine and uses less RAM. Effectively a single-model setup, just with two parallel orchestrators.
- If they differ, Ollama keeps both models loaded (or swaps based on `OLLAMA_MAX_LOADED_MODELS`).

So the two-CLI shape is the *capability* enabling per-role model choice. The user (or default config) decides whether to *use* that capability — it's not forced.

### Ecosystem alignment — what people pair with each agent CLI

We checked Pi, OpenCode, Aider, and Goose to see what runtime they recommend. Universal answer: **Ollama**.

- **Pi**: pi-mono's own [`docs/models.md`](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/models.md) features Ollama first; the `pi-llama-cpp` extension exists for power users on llama-server; LM Studio is the GUI option. Ollama also has a [dedicated Pi integration page](https://docs.ollama.com/integrations/pi) that auto-installs and pre-configures both.
- **OpenCode**: "Ollama is the most common choice for solo coding setups." (multiple comparison articles)
- **Aider**: tutorials default to Ollama; Aider works directly with its OpenAI-compat API.
- **Goose**: documented air-gapped path is "Goose plus Ollama"; the goose-docs.ai site features it prominently.

Migration pattern (across all four): developers start on Ollama for setup ease, graduate to raw llama.cpp only when they're running 500+ inferences/day and the 15–25% speed gap matters. For BoBe's intermittent background workers, that gap is irrelevant.

**Bundling Ollama matches the muscle memory of every user who's looked at any local-AI tutorial in 2026.** Bundling something else means our docs / first-run experience diverges from every other tool — annoying.

### Comparative benchmarks we found (Pi vs OpenCode)

A community blog ([grigio.org](https://grigio.org/local-harness-benchmark-pi-coding-agent-vs-opencode/)) ran identical local models through Pi and OpenCode. Findings:

- "Same model, better results in Pi. Why? OpenCode's system prompt can hit 10K+ tokens while Pi keeps it under 1,000."
- Pi runs 2–3× faster end-to-end with local models because there's less context overhead.
- One user: "Pi nailed a physics problem in one try. OpenCode couldn't solve it." (same model)

**Implication for BoBe**: prompt size matters a lot at local scale. Copilot CLI's system prompt is ~21 K tokens (heavy by comparison). On a 7B model with 32 K context, that leaves only ~11 K for actual conversation. Recommend ≥64 K context window in user-facing docs.

Mario Zechner (pi author) explicitly built pi to address this: "Some harnesses like opencode support self-hosted models, but it usually doesn't work well — they rely on libraries like the Vercel AI SDK, which doesn't play nice with self-hosted models for some reason, specifically when it comes to tool calling."

### The Copilot CLI HTTP-only constraint

We checked whether the CLI could use anything other than TCP HTTP for inference (UDS, named pipes, shared memory). **Answer: no.** `COPILOT_PROVIDER_BASE_URL` accepts only `http://` and `https://`. The CLI does have a separate `--acp --stdio` mode, but that's for *other tools to drive the CLI*, not for the CLI to call inference backends.

This rules out an in-process FFI + UDS optimization. The TCP loopback between Copilot CLI and Ollama is unavoidable — but at ~50 µs vs seconds of inference, it's irrelevant. BoBe is *not* in the request path.

### Reddit research workaround

Reddit is blocked from Anthropic's WebSearch and WebFetch (per their [crawler policy](https://support.anthropic.com/en/articles/8896518)). `web.archive.org` is also blocked. Most libreddit / redlib privacy mirrors are dead since Reddit's 2023 API kill.

**Working workflow**: WebFetch on `https://search.brave.com/search?q=site%3Areddit.com+<terms>`. Brave's snippets surface enough Reddit content to synthesize an opinion summary. Beware: Brave rate-limits parallel requests aggressively (429 after ~3 in a row); serial works.

For Hacker News, `https://hn.algolia.com/api/v1/search?query=X&tags=story` returns JSON directly, no auth, no blocking.

---

## Architectural decisions

### What we picked

| Decision | Rationale |
|---|---|
| **Stay on `github-copilot-sdk`** as the only agent loop | Mature, MS-maintained. Skills + hooks + autopilot + MCP + tool dispatch already integrated and tested in BoBe. Switching loops would mean re-implementing infrastructure we just built (~3K lines). |
| **Bundle the CLI binary** via `bundled-cli` feature | No "is Copilot CLI installed?" failure mode. Cached extraction is integrity-checked and self-repairing. |
| **Two engine modes** (`copilot_cloud` default, `local`) | Cleanest split: one user choice, daemon configures CLI accordingly. Auth, model, base-URL all derive from this single discriminator. |
| **Ollama as the local runtime** | Community standard for Pi, OpenCode, Aider, Goose. Native multi-model + smart scheduler + model registry (`ollama pull`). Bundling Ollama matches the muscle memory of the entire local-AI tutorial ecosystem. |
| **Download Ollama on demand** (don't bundle) | Bundling forces lockstep version pinning with our releases, inflates every BoBe download by ~250 MB even for cloud-only users, and complicates the .app's third-party-binary story. Pre-pivot pattern from `main` already shows how to do this cleanly. |
| **Detect existing Ollama on `:11434`** before spawning ours | Power users who already run Ollama see no second copy. Their `~/.ollama/models` cache is reused — models they've previously pulled don't re-download. |
| **One shared Copilot CLI subprocess** | SDK 1.0 supports per-session model/provider configuration and model overrides on resume. |
| **Default models**: Qwen 2.5 7B Instruct + Qwen 2.5-VL 7B | Both ~5 GB at Q4_K_M. Total local-mode footprint ~10 GB models + ~10 GB resident. Strong text quality + strong vision. *Could default to a single VL model for ~5 GB total* — see open question below. |
| **`Attachment::Blob` for images** (in-memory base64) | Already what BoBe uses. SDK passes via JSON-RPC pipe to CLI subprocess; CLI base64-decodes and forwards to the multimodal API. No disk roundtrip needed. |
| **BoBe never proxies inference calls** | Daemon sets env vars at Client construction, period. SDK + spawned CLI handle every call to GitHub or Ollama. Less code, fewer failure modes. |
| **Hot-swap engine + provider fields** | Config changes rebuild the client; model-only changes preserve resumable session IDs. |

### What we rejected and why

- **`pi_agent_rust`** — single contributor, bus-factor risk for a daemon shipped to users. The crate is sound but ecosystem-fragile.
- **`goose`** — not on crates.io (the name is taken by an unrelated load-testing tool). git deps degrade `cargo audit`/CI semantics. Strong fit architecturally (Rust crate, MCP-native, AAIF-governed) but distribution gap is real today.
- **`opencode-sdk-rs`** — HTTP+SSE topology means spawning `opencode serve` as a sidecar. Doubles architectural surface vs reusing our existing CLI-subprocess pattern.
- **`claude-agent-sdk-rs` + LiteLLM redirect** — vendor-locked SDK abused to drive non-Anthropic models. Reddit reports it works but is fragile (undocumented `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, beta-header rejection). Not a user-facing path.
- **In-process FFI to llama.cpp** (`llama-cpp-2`) — saves ~50 µs of TCP loopback (irrelevant vs seconds of inference). Costs: cmake + Metal toolchain in BoBe build, model lifecycle subsystem in BoBe, in-process model crashes take BoBe down. Net negative.
- **Abandoning the Copilot agent loop** for direct llama.cpp FFI — would mean re-implementing skills, hooks, autopilot, MCP, tool dispatch, JSON repair, multi-turn iteration. ~3K lines of work to break even with what we have today.
- **Bundling `llama-server` + `llama-swap`** — we'd be reinventing Ollama (download + cache + scheduler) for marginal speed gain (~15–25%) on a workload (background workers) where it's irrelevant.
- **Bundling LlamaEdge** — smallest at 30 MB but no native multi-model, smaller community, would still need `llama-swap` on top. The 60 MB savings vs alternatives is dwarfed by 10 GB of model weights.
- **Bundling Ollama in the .app** — forces version lockstep with our releases, inflates download for cloud-only users, complicates the third-party-binary story. Download-on-demand is cleaner.
- **Single VL model for everything by default** — at 7B scale, dedicated text models beat VL counterparts on text. We default to two models for quality; users can configure one if they want.
- **Disk-backed image attachments** — `Attachment::Blob` (in-memory base64) is already optimal. Disk roundtrip is strictly worse.
- **In-process HTTP shim with FFI** (the loopback-with-FFI hybrid I proposed at one point) — strictly more complex than running Ollama as a subprocess; the TCP between CLI and inference backend is unavoidable; trying to "save" it inside our process buys nothing.

---

## Final architecture

```
┌──────────────────────────────────────────────────────────────────┐
│ BoBe daemon (Rust binary, ~120 MB after stripping)               │
│                                                                  │
│   github-copilot-sdk 1.0.6 (with bundled-cli feature)            │
│   ├─ extracts copilot CLI to ~/.cache/github-copilot-sdk-{ver}/  │
│   ├─ ONE shared Client + ONE CLI subprocess                      │
│   └─ Per-session model + provider via SessionConfig.with_model   │
│      / with_provider                                             │
│                                                                  │
│   ConfigManager listens for engine.* PATCH                       │
│   └─ on change: registry.reload() → drops sessions, stops CLI,   │
│      forgets session IDs → next worker access re-spawns          │
│                                                                  │
│   binary_manager / ollama_manager / OllamaInstallService         │
│   ├─ probes localhost:11434 → uses user's Ollama if present      │
│   ├─ else downloads ollama-darwin.tgz → /api/pull each model     │
│   └─ progress streamed on watch::Sender<InstallSnapshot>          │
│                                                                  │
│   Endpoints:                                                     │
│   ├─ GET /auth/status     → wraps Client.get_auth_status         │
│   ├─ GET /models?engine=… → cloud: SDK list_models;              │
│   │                          local: Ollama /api/tags             │
│   ├─ POST /local-runtime/install + /local-runtime/cancel          │
│   └─ GET /local-runtime/status (SSE)                             │
│                                                                  │
└──┬───────────────────────────────────────────────────────────────┘
   │ stdio JSON-RPC
   ▼
┌─────────────────────────────────┐
│ ONE Copilot CLI subprocess      │
│                                 │
│  Sessions configured per-class: │
│    Chat        → chat_model     │  ──→  HTTPS (cloud) or
│    Goals       → batch_model    │       http://127.0.0.1:11434/v1
│    Decide      → batch_model    │       (local)
│    Consolidate → batch_model    │
│    Vision      → vision_model   │
└──┬──────────────────────────────┘
   ▼
GitHub Copilot   ─OR─   Ollama (managed by us, or user's existing)
                            │
                            ▼
                       Qwen 2.5 7B + Qwen 2.5-VL 7B
                       (default models; user can change in Settings → Engine)
```

Worker-class routing (uniform across cloud / local):
- **Chat** → `provider_chat_model` (the user-facing dialogue model)
- **Goals / Decide / Consolidate** → `provider_batch_model` (autopilot batch jobs — can be cheaper / faster)
- **Vision** → `provider_vision_model` (capture pipeline — must support image inputs in local mode)

If `chat_model == batch_model == vision_model`, the CLI session config is identical across all five workers and Ollama keeps one model resident. If they differ, Ollama's `OLLAMA_MAX_LOADED_MODELS` keeps as many resident as fit.

---

## Phased plan — historical reference

> **Note:** All phases listed below are now **shipped**. Sketches preserved here for cross-reference with commit history. See [What's been shipped](#whats-been-shipped) for the actual implementation summaries.

### Phase 5a — Resurrect `binary_manager` (#58)

Bring back `BoBeService/src/binary_manager/{mod,download,extract}.rs` from `main` (~460 lines total). Simplifications:
- Drop multi-arch matrix → macOS arm64 + x64 only
- Drop the `OllamaProvider` glue → only `ensure_managed_ollama` survives
- Re-add `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "stream"] }` to `Cargo.toml`

Public API:
```rust
pub(crate) struct BinaryManager { /* data_dir, http_client */ }

impl BinaryManager {
    pub(crate) fn new(data_dir: &Path, http: Arc<reqwest::Client>) -> Self;

    /// Returns path; downloads to ~/.bobe/runtimes/ollama if absent
    /// or hash-mismatched. Idempotent.
    pub(crate) async fn ensure_managed_ollama(
        &self,
        progress_tx: &watch::Sender<DownloadProgress>,
    ) -> Result<PathBuf, AppError>;
}
```

Tests: download stub against a local file URL; SHA-256 mismatch returns `AppError::Integrity`.

### Phase 5b — Resurrect `ollama_manager` + runtime service + two-Client topology (#59)

Two pieces, one commit (tightly coupled).

**`copilot/ollama_manager.rs`** (~200 lines, restored from main, simplified):

```rust
pub(crate) struct OllamaManager { /* http_client */ }

impl OllamaManager {
    pub(crate) async fn detect_running(&self) -> bool;
    pub(crate) async fn ensure_daemon_running(
        &self,
        binary_path: Option<&Path>,
        auto_start: bool,
    ) -> Result<(), AppError>;
    pub(crate) async fn pull_model(
        &self,
        name: &str,
        progress_tx: &watch::Sender<DownloadProgress>,
    ) -> Result<(), AppError>;
    pub(crate) async fn list_models(&self) -> Result<Vec<String>, AppError>;
}
```

**`services/ollama_runtime_service.rs`**: orchestrates the binary + manager. End-to-end first-launch flow for local mode:
1. Probe `:11434` — if responding, skip download
2. `binary_manager.ensure_managed_ollama` (with `progress_txs[0]`)
3. `ollama_manager.ensure_daemon_running`
4. `ollama_manager.list_models` — diff against required set
5. `pull_model × N` in parallel for missing ones (`progress_txs[1]`, `[2]`)

**Two-Client topology in `copilot/{client,registry}.rs`:**

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

`client_options_for_role(cfg, role)` reads `Config.engine` and assembles env vars.

### Phase 5c — `/local-runtime/install` + `/local-runtime/status` SSE (#60)

New axum routes:
- `POST /local-runtime/install` — body `{ text_model, vision_model }`, kicks off `OllamaRuntimeService::install`, returns 202
- `GET /local-runtime/status` (SSE) — streams `{ stage: "ollama" | "text-model" | "vision-model", percent, bytes_done, bytes_total, complete: bool, error?: string }`
- `POST /local-runtime/cancel` — aborts in-flight install

Concurrent installs return 409. Daemon stores the in-flight task handle.

### Phase 5d — Wizard local-card download UI (#61)

After `EngineChoiceStepView` selection of "local", wizard advances to `LocalSetupStepView`:

1. Hardware check via `sysctl hw.memsize`. Warn if `<16 GB`; recommend `32 GB+`. Don't block.
2. Three stacked `BobeLinearProgressBar` instances driven by SSE `stage`:
   - Ollama runtime
   - Text model
   - Vision model
3. "Cancel" button → `POST /local-runtime/cancel`, returns to engine choice
4. On all-three-complete → wizard `PATCH /settings` with `{engine: "local", provider_base_url: ..., provider_text_model: ..., provider_vision_model: ...}`, advances to permissions step

Local-card flow takes ~8–15 minutes on a fast connection. UI must remain responsive; user can switch away and back.

### Phase 3 — Settings Engine pane (#52)

New top-level Settings category `Engine` (or fold into `Advanced` if 9 is the cap — decide during impl).

UI elements:
- Radio: Cloud / Local
- (Local) Editable fields: provider_base_url, provider_text_model, provider_vision_model
- (Local) Toggle: provider_offline
- (Cloud) "Sign in to GitHub Copilot" button only when not authed
- Restart-required banner shadow set extends with the 5 engine fields

### Phase 4 — Returning-user migration (#53)

Existing users have `bobe.onboarding_completed = true`; they won't see the new wizard. Two-line fix:
- Daemon defaults to `engine: "copilot_cloud"` on first read after upgrade (already in Phase 1's defaults)
- Add a one-time non-blocking pill if user opens Settings → Engine and `engine_choice` UserDefault is unset → "Welcome to local-AI mode! Set up here." Otherwise no nudge — behavior matches today.

### Phase 2b — Cloud-card auth-status check (#57)

After cloud-card selection, wizard advances to `AuthCheckStepView`:
- Daemon endpoint `GET /auth/status` calls SDK's `client.get_auth_status()`
- If `is_authenticated`: green check, "Signed in as @{login}" → continue
- If not: "Sign in" button shells out to `copilot login` in Terminal.app via `NSWorkspace.shared.open`. "Retry check" re-probes. "Skip" advances anyway (user discovers auth issue on first chat with a clear error).

---

## Default models + hardware floor

| Slot | Default model | Quantization | Disk | Loaded RAM | Notes |
|---|---|---|---|---|---|
| Text | `qwen2.5:7b-instruct` | Q4_K_M | ~5 GB | ~4–5 GB | Strong text quality, great `--jinja` tool calling |
| Vision | `qwen2.5-vl:7b` | Q4_K_M | ~5 GB | ~4–5 GB | Excellent image understanding; cleanly handles text-only requests too |

**First-launch download**: ~250 MB Ollama binary + ~10 GB models = ~10.25 GB total.

**RAM floor for local mode** (sum of resident processes):
- BoBe daemon: ~120 MB
- 2× Copilot CLI subprocesses: ~430 MB
- Ollama: ~50 MB idle; +10 GB for both models loaded
- macOS overhead + user apps: ~4 GB
- **Total**: ~14.5 GB → realistically need **16 GB minimum, 32 GB recommended**.

If user RAM-constrained, they can configure both Copilot Clients to point at the same model (`provider_text_model = provider_vision_model = qwen2.5-vl:7b`) — then Ollama keeps only one model resident, saving ~5 GB. UI in Settings → Engine should expose this clearly.

Wizard surfaces a warning if `sysctl hw.memsize` returns < 16 GB. User can proceed but is informed.

---

## Open questions

- **Hot-swap timing edge case**: `WorkerRegistry::reload()` shuts down sessions + stops the CLI + forgets session IDs. If a worker is mid-turn when the user toggles the engine in Settings, that turn's response stream cancels. Acceptable trade-off for "no daemon restart" — but worth a smoke test (#28) to verify the user-visible failure mode is clean.
- **First chat after engine swap**: chat session ID is forgotten on reload, so the chat resumes "fresh" in the new engine — yesterday's history is gone. This is intentional (the model changed, the prior history was generated by a different model). Surface a "your chat history reset because the model changed" notice on the next turn? Defer.
- **Default model size**: 5 GB × 2 = 10 GB download. Some users on metered connections will resent this. Consider "smaller models — needs less disk" option behind an advanced toggle. Possible smaller default: `qwen2.5:3b-instruct` + `qwen2.5vl:3b` (~4 GB total) for resource-constrained hardware.
- **Single VL model as default?**: an alternative — set `chat_model = batch_model = vision_model = qwen2.5vl:7b`, half the disk + RAM cost. Text quality is slightly worse than Qwen 2.5 7B Instruct at the 7B class, but simpler. Watch user feedback after the first wave of local-mode users.
- **Qwen 2.5-VL 7B text quality vs Qwen 2.5 7B**: claimed gap but no clean head-to-head benchmark. Empirical question — verify before locking the default.
- **CLI version pinning**: `COPILOT_CLI_VERSION = "1.0.44"` is hand-set. Need a process for upstream releases — eventually wire to dependabot or a CI alert.
- **Cache cleanup on Copilot CLI upgrade**: when we bump `COPILOT_CLI_VERSION`, the old `~/.cache/github-copilot-sdk-{old-ver}/` dir lingers. One-time cleanup task on first run after upgrade.
- **Ollama auto-update**: managed-by-BoBe Ollama doesn't auto-update (user's brew-installed Ollama would). Surface "newer Ollama available" in Settings → Engine, prompt to re-run `binary_manager` with new version. Defer until users hit it.
- **Cancellation mid-pull**: if user cancels during `ollama pull`, a partial blob is left in `~/.ollama/models`. Ollama tolerates this on retry but it takes disk. Background task to clean partial pulls? Defer.
- **No clear "uninstall local mode" flow** — switching back to cloud leaves 10 GB of models in `~/.ollama/`. Surface a "Free disk space" button in Settings → Engine that calls `ollama rm qwen2.5:7b qwen2.5vl:7b`?
- **Subscription gate detection** for cloud is impossible pre-auth. Best we can do: warn in cloud-card copy ("requires Copilot subscription"), fail gracefully on first call. Document the failure mode.
- **Live MCP runtime state** (#31): the MCP panel hides `connected` / `tool_count` today — values are stubbed. SDK doesn't yet expose a sideband query for live MCP server state. Track upstream.
- **Smoke test** (#28): bring the daemon up live, exercise each flow end-to-end. The architecture is sound but hasn't been validated against a running stack since the foundation rev.

---

## How to research things in this space

For future-us picking up this thread:

- **Ecosystem moves fast.** The BYOK env vars shipped April 2026, Pi was first-released late 2025, Goose moved to AAIF in late 2025, llama.cpp router mode landed mid-2024. Check current state before assuming.
- **Reddit is gated.** Use Brave Search via `WebFetch` on `search.brave.com/search?q=site%3Areddit.com+...` to surface snippets. Sequential calls only (Brave 429s on parallel). Hacker News is open via `hn.algolia.com/api/v1/search?query=...&tags=story` JSON API.
- **Three-layer mental model**: SDK / agent loop / model runtime. Don't conflate them. A discussion about "should we use Ollama or pi?" is conflating Layer 1 with Layer 2.
- **Bundle size claims should be re-checked per platform.** "Ollama is 4.6 GB" was a Windows install with CUDA + ROCm. The macOS install is small.
- **github/copilot-sdk** monorepo has language subdirs (rust, python, go, dotnet, java, nodejs). Different release trains; check `rust/CHANGELOG.md` and `rust/Cargo.toml` for current state.
- **Context7 (`mcp__plugin_context7_*`)** is great for library docs but doesn't have community sentiment. Use Brave + HN Algolia for that.

---

## Sources

### Copilot CLI / SDK
- [GitHub Changelog: Copilot CLI BYOK + local models (2026-04-07)](https://github.blog/changelog/2026-04-07-copilot-cli-now-supports-byok-and-local-models/)
- [GitHub Docs: BYOK env vars](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-byok-models)
- [Ollama Copilot CLI integration](https://docs.ollama.com/integrations/copilot-cli)
- [github/copilot-sdk monorepo](https://github.com/github/copilot-sdk)
- [Issue #248 — bundling discussion](https://github.com/github/copilot-sdk/issues/248) and [Steve Sanderson's confirmation](https://github.com/github/copilot-sdk/issues/248#issuecomment-3813716744)
- [`rust/build.rs` — bundling impl](https://github.com/github/copilot-sdk/blob/main/rust/build.rs)
- [`rust/src/embeddedcli.rs` — runtime extract](https://github.com/github/copilot-sdk/blob/main/rust/src/embeddedcli.rs)
- [`rust/src/resolve.rs` — binary resolve order](https://github.com/github/copilot-sdk/blob/main/rust/src/resolve.rs)
- [Copilot CLI releases](https://github.com/github/copilot-cli/releases)
- [Issue #2531 — local AI model support history](https://github.com/github/copilot-cli/issues/2531)
- [Mainbranch: BYOK with Ollama + Gemma](https://mainbranch.dev/articles/copilot-cli-byok-ollama/)
- [Laminar: instrumenting Claude Agent SDK with a Rust proxy](https://laminar.sh/blog/2025-12-03-claude-agent-sdk-instrumentation) — useful pattern for subprocess-based SDK observability

### Local runtimes
- [Ollama macOS docs](https://docs.ollama.com/macos)
- [Ollama FAQ — concurrency + multi-model](https://docs.ollama.com/faq)
- [Ollama new model scheduling](https://ollama.com/blog/new-model-scheduling)
- [llama.cpp tools/server README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [llama.cpp function-calling docs](https://github.com/ggml-org/llama.cpp/blob/master/docs/function-calling.md)
- [llama.cpp router mode (HF blog)](https://huggingface.co/blog/ggml-org/model-management-in-llamacpp)
- [llama-swap GitHub](https://github.com/mostlygeek/llama-swap)
- [llama-swap setup guide 2026](https://modelslab.com/blog/api/hot-swap-local-llms-instantly-llama-swap-setup-guide-2026)
- [LlamaEdge GitHub](https://github.com/LlamaEdge/LlamaEdge) and [LlamaEdge vs Ollama](https://llamaedge.com/docs/llamaedge_vs_ollama/)
- [Anthropic Messages API in llama.cpp](https://huggingface.co/blog/ggml-org/anthropic-messages-api-in-llamacpp)
- [Unsloth: llama-server + OpenAI endpoint deployment](https://unsloth.ai/docs/basics/inference-and-deployment/llama-server-and-openai-endpoint)

### Models — Qwen 2.5/3, vision-language landscape
- [Qwen 2.5-VL 7B (Qwen blog)](https://qwen.ai/blog?id=qwen2.5-vl)
- [Qwen 2.5: foundation models](https://qwenlm.github.io/blog/qwen2.5/)
- [Qwen3 technical report (PDF)](https://arxiv.org/pdf/2505.09388)
- [Qwen3-VL usage guide (vLLM)](https://docs.vllm.ai/projects/recipes/en/latest/Qwen/Qwen3-VL.html)
- [Top open-source vision-language models in 2026 (BentoML)](https://www.bentoml.com/blog/multimodal-ai-a-guide-to-open-source-vision-language-models)
- [Qwen 2.5-VL vs Llama 3.2 Vision](https://www.labellerr.com/blog/qwen-2-5-vl-vs-llama-3-2/)
- [Best LLMs for OpenCode tested locally (Glukhov)](https://www.glukhov.org/ai-devtools/opencode/llms-comparison/)

### Agent CLI ecosystem (Pi, OpenCode, Aider, Goose)
- [pi-mono GitHub](https://github.com/badlogic/pi-mono)
- [pi-mono coding-agent docs](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/)
- [pi-mono local LLM models doc](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/models.md)
- [Ollama Pi integration](https://docs.ollama.com/integrations/pi)
- [pi_agent_rust (Rust port)](https://github.com/Dicklesworthstone/pi_agent_rust) and [crates.io v0.1.7](https://crates.io/crates/pi_agent_rust)
- [Mario Zechner: building pi-coding-agent](https://mariozechner.at/posts/2025-11-30-pi-coding-agent/)
- [OpenCode docs](https://opencode.ai/docs/)
- [opencode-sdk-rs](https://docs.rs/opencode-sdk-rs/latest/opencode_sdk_rs/)
- [block/goose (now under AAIF)](https://github.com/block/goose) and [Goose AGENTS.md](https://github.com/block/goose/blob/main/AGENTS.md)
- [Aider chat](https://aider.chat)

### Comparison and benchmarks
- [grigio.org: OpenCode vs Pi which to use](https://grigio.org/opencode-vs-pi-which-ai-coding-agent-should-you-use/)
- [grigio.org: Local Harness Benchmark — Pi vs OpenCode](https://grigio.org/local-harness-benchmark-pi-coding-agent-vs-opencode/)
- [grigio.org: OpenCode vs Pi local LLM benchmarks](https://grigio.org/opencode-vs-pi-local-llm-benchmark-results/)
- [thoughts.jock.pl: Which AI coding harness actually works without you?](https://thoughts.jock.pl/p/ai-coding-harness-agents-2026)
- [Tembo: 2026 guide to coding CLI tools (15 agents compared)](https://www.tembo.io/blog/coding-cli-tools-comparison)
- [Pinggy: Top 5 CLI coding agents 2026](https://pinggy.io/blog/top_cli_based_ai_coding_agents/)
- [The Register: How to roll your own local AI coding agents](https://www.theregister.com/2026/05/02/local_ai_coding_agents/)
- [Tildes: Is it worthwhile to run local LLMs for coding today?](https://tildes.net/~comp/1t2j/is_it_worthwhile_to_run_local_llms_for_coding_today)
- [bitdoze: OpenCode vs Pi Agent](https://www.bitdoze.com/opencode-vs-pi-agent/)
- [Openxcell: llama.cpp vs Ollama 2026](https://www.openxcell.com/blog/llama-cpp-vs-ollama/)
- [Red Hat: vLLM vs llama.cpp inference engines](https://developers.redhat.com/articles/2025/09/30/vllm-or-llamacpp-choosing-right-llm-inference-engine-your-use-case)
- [DEV: Why Ollama overhead exists](https://dev.to/plasmon_imp/ollama-lm-studio-and-gpt4all-are-all-just-llamacpp-heres-why-performance-still-differs-59h5)
- [Mastering multi-model stacks with llama-swap](https://dasroot.net/posts/2026/05/mastering-multi-model-stacks-llama-swap/)

### Rust ML ecosystem (for context — we're not using these)
- [llama-cpp-2 on crates.io](https://crates.io/crates/llama-cpp-2)
- [llama_cpp safe high-level bindings](https://docs.rs/llama_cpp)
- [llama-cpp-4 (newer fork)](https://crates.io/crates/llama-cpp-4)
- [mistral.rs (Candle-based, multimodal)](https://github.com/EricLBuehler/mistral.rs)
- [Hugging Face Candle](https://github.com/huggingface/candle)
- [ushi — production llama.cpp inference server in Rust](https://lib.rs/crates/ushi)

### Process / research workflow
- [Anthropic crawler policy (why Reddit can't be fetched directly)](https://support.anthropic.com/en/articles/8896518)
- [HN Algolia API](https://hn.algolia.com/) — JSON, no auth, surfaces dev opinion threads

---

## Cross-references

- This file: `ENGINE_PROVIDER_NOTES.md` (repo root) — single source of truth
- Earlier SwiftUI catch-up plan: `SWIFTUI_CATCHUP_PLAN.md` — completed work that landed before the engine work
- Memory: `/Users/john/.claude/projects/-Users-john-Repos-bobrust/memory/project_copilot_workers_initiative.md`
- Daemon source layout: `BoBeService/src/{copilot,api,config,config_manager,services}/`
- Swift source layout: `BoBeMacUI/BoBe/{Models,Services,Stores,Views/Setup,Features/Settings}/`

---

*Generated 2026-05-10. The plan-of-record for engine + provider architecture on this branch.*
