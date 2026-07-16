# Second Audit Fix Set

Date: 2026-07-16
Branch: `fix/full-codebase-audit`

This file supplements `docs/whole-codebase-audit-fixes.md` with the broader lifecycle, migration, release, dead-code, resource, and split-brain review requested afterward.

## Repository authority

- No separate feature work superseded this branch.
- `origin/main` was 239 commits behind the audit branch when checked.
- Historical agent worktrees based on `5479298` were not newer product work; that merge is an ancestor of `042a610`.
- Agent changes were integrated by task-scoped three-way merges. Old pre-pivot config and binary-manager layouts were explicitly not restored.

## Migration and data safety

- Replaced unconditional legacy table drops with a versioned migration.
- Added durable JSON backup and verification before destructive migration work.
- Migrated legacy goals, plans, plan steps, memories, and observations to current Markdown-backed storage.
- Preserved plan details in migrated goal documents.
- Verified output files before dropping legacy tables transactionally.
- Made the migration idempotent and added a published-schema fixture test.
- Established `Config.data_dir` as the canonical root for SQLite defaults, memory, goals, workers, skills, models, MCP configuration, migration backups, and tool output.
- Kept an explicitly configured database URL as the only override.
- Added shared durable writes with file and parent-directory synchronization.
- Added durable deletion for critical goal/session/worker paths.
- Replaced destructive Keychain delete-then-add with `SecItemUpdate`, falling back to add only when absent.

## Privacy deletion

- Added daemon-owned `DELETE /privacy/data` coordination.
- Purge rejects active turns.
- Destroys current Copilot SDK sessions and clears persisted worker IDs.
- Clears conversations, turns, cooldowns, memory, goals, custom Souls/Profiles, MCP configuration, MCP secrets, and tool output.
- Retains and explicitly reports settings/API credentials, installed models/runtimes, default documents, and Copilot login.
- Swift Privacy settings now performs one authoritative purge request rather than several partial client-side deletes.

## Security and remote deployment

- Added exact WebSocket Origin validation for browser voice connections.
- Preserved bearer authentication across REST, all SSE streams, and voice WebSocket requests.
- Added constant-time bearer-token comparison.
- Non-loopback or public-host/reverse-proxy configuration requires token and TLS configuration.
- Constructed listener addresses structurally from `IpAddr` and port, including IPv6 loopback.
- Preserved remote URL path prefixes for voice WebSocket endpoints.
- Removed verbatim user transcript, wake phrase, cancel phrase, and filler text from production logs.
- Constrained voice-model targets beneath the configured models root.
- Updated remote/reverse-proxy documentation.

## Linux support

- Added Ubuntu locked check/build/test CI.
- Added exact Rust 1.94 MSRV CI and local validation.
- Added platform-specific managed Ollama artifacts:
  - macOS: official `ollama-darwin.tgz` with pinned checksum and bound;
  - Linux x86_64: official `ollama-linux-amd64.tar.zst` with pinned checksum and 1.5 GB bound.
- Added zstd archive extraction.
- Unsupported platform tuples now fail explicitly instead of downloading Darwin binaries.
- Added platform artifact tests.

## Runtime split-brain cleanup

- Enforced conversation inactivity at shared proactive admission using the latest persisted user turn.
- Admission failures fail closed.
- `mcp_enabled` now hot-applies the effective MCP runtime map and serializes with config document changes.
- Removed the contradictory Swift restart-required workaround for MCP enablement.
- Successful settings persistence immediately applies committed voice settings to `VoicePipeline`.
- Disabling voice disconnects an active session only after persistence succeeds.
- Active sessions freeze language, TTS backend, persona, and speed; edits apply to the next session.

## Streaming, cancellation, and resource bounds

- Terminal SSE text events carry the authoritative full assistant response.
- Swift replaces partial accumulated content on terminal completion, preventing queue-overflow truncation.
- Added cross-language SSE integrity tests.
- Voice writer drain timeout now aborts and awaits the writer instead of detaching it.
- Ollama model pulls and binary downloads race connection and stalled reads against cancellation.
- Partial files are deleted on cancel.
- Swift chat state is bounded to 200 messages while preserving pending/streaming messages.
- Expanded chat uses `LazyVStack`.
- Closed conversations are pruned in bounded batches using defaults of 90 days and 1,000 records.
- Daily logs are bounded to 14 days, 14 files, and 100 MiB.
- Retention deletes only closed conversations and BoBe-owned regular log files, preserving active data, unrelated files, symlinks, and the current log.
- Retention runs at startup and every six hours.

## macOS lifecycle

- Incomplete onboarding cannot silently close into a disconnected overlay.
- Closing onboarding keeps setup visible or offers Quit.
- Activation, reopen, menu, and menu-bar actions reopen onboarding until completion.
- Going Back from local setup offers keep-download or cancel-download behavior.
- Local-runtime SSE reconnects and recovers current watch-channel state.
- Copilot login reconnects, checks final auth status, and surfaces unknown phases as compatibility failures.
- Voice recovery opens only `.voiceSetup` and does not reapply onboarding engine/personalization.
- Locale changes no longer remount the complete Settings hierarchy, preserving dirty editor state.

## Release and CI

- CI path filters now include all workflows, `justfile`, scripts, and lockfiles.
- Added dependency-free workflow YAML and shell syntax validation.
- Added Ubuntu stable Rust locked check/build/test.
- Added exact Rust 1.94 MSRV validation.
- Release workflow now describes the appcast operation truthfully as creating a website deployment PR rather than claiming public deployment.
- `BoBe.app` is stapled and validated before Sparkle ZIP creation.
- Extracted Sparkle archives are checked with `codesign` and stapler validation.
- Fixed `just bundle` to embed every SwiftPM dynamic framework required by `@rpath`.
- Bundle assembly now fails if a linked framework is absent.
- This fixed a real dyld launch crash caused by missing `Sparkle.framework`.

## Dead-code and split-brain result

- Strict Rust Clippy dead-code/unused scans found no compiler-confirmed Rust dead code.
- The abandoned pre-pivot `binary_manager` stash was not applied; Linux support was implemented in the current `services/ollama` architecture.
- The old monolithic `config.rs` and `config_manager/` retention patch was not restored; retention was ported into current modular `config/` files.
- Semantic dead/no-op paths fixed:
  - conversation inactivity setting;
  - MCP enabled state;
  - stale voice settings mirror;
  - incomplete privacy deletion;
  - stale local-runtime SSE state;
  - unknown Copilot phases;
  - Darwin-only Linux installer path.

## Final validation

### Static and automated

- `just check`: passed on the final integrated primary tree.
- Rust: 217 tests passed before the final two retention and platform tests; focused final tests also passed.
- Swift: 43 tests across 12 suites passed.
- Rust formatting and Clippy passed.
- SwiftLint and Swift debug build passed.
- `cargo deny` and `cargo machete` passed, with existing informational duplicate/yanked transitive warnings.
- Swift runtime artifact and Rust/Swift contract checks passed.
- `cargo +1.94.0 check --locked`: passed.
- Release build produced `build/BoBe.app`.

### Runtime

Using isolated data directories and alternate ports:

- Hostile voice WebSocket Origin returned HTTP 403.
- Trusted configured Origin returned HTTP 101.
- Privacy purge returned truthful deletion counts and retained-data list.
- Final bundle initially exposed missing Sparkle embedding; the bundle recipe was fixed and revalidated.
- Final `BoBe.app` launched successfully with embedded Sparkle.
- Onboarding rendered with a populated accessibility tree.
- Command-W did not hide or strand incomplete onboarding.
- Quitting the app stopped its owned daemon.

### Generated artifacts

- Peekaboo screenshots and JSON snapshots were removed after inspection.
- No commits or pushes were created.
