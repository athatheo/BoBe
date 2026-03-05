# Rust Code Organization & Architecture Guidelines

Compiled from: Rust API Guidelines, Effective Rust, Clippy docs, Tor coding standards, corrode.dev, community consensus.

---

## Key Thresholds

| Metric | Target | Smell |
|--------|--------|-------|
| File size | 300-500 lines | >1,000: must split |
| Function size | <50 lines logic | >100: must split |
| Cognitive complexity | <15 | >25: Clippy default |
| Function params | ≤5 | >7: Clippy default |
| Module nesting | 2-3 levels | >4: separate crate |
| Struct fields | <7-10 | >10: consider splitting |

---

## SOLID in Rust

- **SRP**: One responsibility per struct/module/function. Split when methods are unrelated.
- **OCP**: Use traits for extension without modification.
- **LSP**: Trait impls must honor the semantic contract, not just the signature.
- **ISP**: Small, focused traits. Compose with `T: Read + Write`.
- **DIP**: Depend on traits, not concrete types. Constructor injection via trait bounds.

---

## Design Principles

- **Parse, Don't Validate**: Parse into types that can only exist if valid (e.g., `Email(String)` with private field + `parse()` constructor).
- **Make Illegal States Unrepresentable**: Use enums so invalid combinations can't be constructed.
- **Newtype Pattern**: Wrap primitives for type safety (`UserId(u64)` vs `OrderId(u64)`), orphan rule bypass, invariant encapsulation.
- **Typestate Pattern**: Encode state in the type system. Start with enums; move to typestate when the state diagram is stable.
- **Ownership-Driven Design**: Single owner → borrow → `Rc`/`Arc` → interior mutability, in that order of preference.

---

## Common Pitfalls

The borrow checker does NOT catch:

- **Silent truncation with `as`**: Use `TryFrom`/`TryInto` instead. Enable `cast_possible_truncation` lint.
- **Integer overflow**: Panics in debug, wraps in release. Use `checked_*`/`saturating_*`.
- **Deadlocks**: Lock ordering bugs, double-locking, holding locks across `.await`.
- **`Rc`/`Arc` cycles**: Use `Weak` to break them.
- **Clone abuse**: Question every `.clone()`. Write `Arc::clone(&x)` to make refcount clones visible.
- **`Path::join` with absolute paths**: Silently drops the base path.
- **Forgetting `.await`**: Silently does nothing.
- **Ignoring `Result`**: `let _ = ...` silently discards errors.

---

## Async Pitfalls

- **MutexGuard across `.await`** (the #1 footgun): Scope the lock, clone what you need, drop, then `.await`. Use `std::sync::Mutex` unless you must hold across `.await`.
- **Blocking the runtime**: No `std::thread::sleep()` or sync I/O in async. Use `spawn_blocking`.
- **Cancellation unsafety**: Any future can be cancelled at any `.await` point. `select!` drops the losing branch.
- **Dropping `JoinHandle`**: Does NOT cancel the task. Use `handle.abort()` or `CancellationToken`.
- **`select!` starvation**: One always-ready branch starves others. Hoist futures outside the loop.

---

## Module Organization

- Group by domain (`services/conversation.rs`) not by type (`structs/conversation_service.rs`)
- Re-export key types at module roots with `pub use`, keep submodules private
- Max 3 levels of nesting; deeper = separate crate

**Note:** This project currently uses `mod.rs` files throughout. The modern `module.rs` + `module/` directory style is preferred for new modules.

### Antipatterns

- **God modules**: Everything in `lib.rs`/`main.rs`
- **Over-modularizing**: One function per file
- **Glob re-exports**: `pub use submod::*` makes tracing impossible
- **Circular module dependencies**: Extract shared types to break cycles
- **Bare `pub` in binary crate**: Always use `pub(crate)` — bare `pub` suppresses dead code detection

---

## Layering (Axum)

```
Handler (api/handlers/) — extract, delegate, respond
    ↓
Service (services/) — business logic, orchestration
    ↓
Repository (db/) — data access only, returns domain models
```

Error conversion at boundaries via `From` impls. Simple CRUD handlers may call repos directly.

---

## Error Handling

- `thiserror` for structured error types with matchable variants
- Layer errors: `RepositoryError → ServiceError → AppError` via `From`
- Preserve error chain (`#[source]` or `#[from]`)
- No `unwrap()` except logically impossible failures (with comment) or tests

---

## Visibility

Binary crate — nothing is exported externally.

- All cross-module items **MUST** use `pub(crate)`, never bare `pub`
- `pub(super)` for module subtree sharing
- `unreachable_pub` lint enforced in `Cargo.toml`

## Dead Code Detection

The compiler automatically detects unused `pub(crate)` items. When a `dead_code` warning appears:

- **Remove** if genuinely unused with no future plan
- **`#[allow(dead_code)]` with comment** if intentionally kept for a planned feature (e.g., `// planned: audio input`)

## Handler Thickness

Handlers should be thin: extract → delegate → respond. Exception for config/settings endpoints with wide field mapping where splitting adds indirection without clarity.

---

## DI Without Framework

- `Arc<dyn Trait>` for services needing testability
- Wire in a single bootstrap module
- `AppState` as DI container (Axum-idiomatic)

---

## Clippy Lints

The project currently enables `clippy::pedantic` and `clippy::all` as warnings with justified allows (see `Cargo.toml`). The following additional lints are recommended but not yet configured:

```toml
# Panic prevention (not yet enabled)
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
indexing_slicing = "warn"

# Debug leftovers (not yet enabled)
dbg_macro = "warn"
todo = "warn"
print_stdout = "warn"
print_stderr = "warn"

# Arithmetic safety (not yet enabled)
arithmetic_side_effects = "warn"

# Clarity (not yet enabled)
clone_on_ref_ptr = "warn"
```

Note: casting lints (`cast_possible_truncation`, `cast_sign_loss`, `cast_possible_wrap`) are explicitly allowed in `Cargo.toml` — all cast sites have been audited.

---

## Sources

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
- [Effective Rust](https://effective-rust.com/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)
- [Clippy Lint Configuration](https://doc.rust-lang.org/nightly/clippy/lint_configuration.html)
- [Pitfalls of Safe Rust — corrode.dev](https://corrode.dev/blog/pitfalls-of-safe-rust/)
- [Luca Palmieri — Error Handling](https://lpalmieri.com/posts/error-handling-rust/)
- [Tokio — Shared State](https://tokio.rs/tokio/tutorial/shared-state)
- [Cancelling Async Rust — sunshowers.io](https://sunshowers.io/posts/cancelling-async-rust/)
