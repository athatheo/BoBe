# Rust Code Organization & Architecture Guidelines

Compiled from: Rust API Guidelines (official), Effective Rust, Clippy docs, Tor coding standards, corrode.dev, community consensus (2025-2026).

---

## Key Thresholds

| Metric | Target | Smell |
| -------- | -------- | ------- |
| File size | 300-500 lines | >1,000: must split |
| Function size | <50 lines logic | >100: must split |
| Cognitive complexity | <15 | >25: Clippy default |
| Function params | ≤5 | >7: Clippy default |
| Bool params | 0-1 per fn | >3: Clippy default |
| Module nesting | 2-3 levels | >4: separate crate |
| Struct fields | <7-10 | >10: consider splitting |

---

## SOLID Equivalents in Rust

Rust isn't OOP, but the goals of SOLID still apply:

### Single Responsibility (SRP)

Each struct, module, and function should have one well-defined responsibility. Rust's module system is the primary mechanism — when a struct accumulates unrelated methods, split into separate modules or traits.

### Open/Closed (OCP)

Use traits for extension without modification. A fn accepting `impl Shape` works with any future type implementing the trait — no changes needed.

### Liskov Substitution (LSP)

Any type implementing a trait must honor the trait's *semantic* contract. The compiler enforces signatures but not behavioral correctness — that's your job.

### Interface Segregation (ISP)

Small, focused traits. Compose with `T: Read + Write`. Don't force implementors to define methods they don't need.

### Dependency Inversion (DIP)

Depend on traits (via generics or `dyn Trait`), not concrete types. Constructor injection via trait bounds.

---

## Rust-Specific Design Principles

### Parse, Don't Validate

Instead of validating raw data and passing it around, parse into a type that **can only exist if valid**. `Email(String)` with a private field + `parse()` constructor beats a bare `String` with a `validate_email()` call.

### Make Illegal States Unrepresentable

Use enums so invalid combinations can't be constructed:

```rust
// BAD: ssl_enabled=true + certificate=None is invalid but constructable
// GOOD: enum ConnectionSecurity { Plaintext, Tls { certificate: String } }
```

### Newtype Pattern

Wrap primitives for compile-time type safety at zero runtime cost:

- Prevents argument mix-ups: `UserId(u64)` vs `OrderId(u64)`
- Orphan rule bypass
- Encapsulates invariants: `NonEmptyVec<T>`

### Typestate Pattern

Encode state in the type system so invalid transitions are compile errors. Start with enums when exploring; move to typestate when the state diagram is stable.

### Ownership-Driven Design

Design data structures around *who owns what*:

1. Can this have a single owner? (move semantics)
2. Can borrowers use references? (`&T`, `&mut T`)
3. Does shared ownership truly require `Rc`/`Arc`?
4. Is interior mutability truly needed? (Only then `RefCell`/`Mutex`)

---

## Common Bugs the Borrow Checker Does NOT Catch

### Silent Truncation with `as`

`as` performs lossy conversions **without warning**:

```rust
let big: u16 = 0x1234;
let small: u8 = big as u8; // silently 0x34!
let neg: i32 = -1;
let unsigned: u32 = neg as u32; // silently 4294967295!
```

**Fix:** Use `TryFrom`/`TryInto`, `checked_*`, `saturating_*`. Enable `cast_possible_truncation`, `cast_sign_loss` clippy lints.

### Integer Overflow

Panics in debug, **silently wraps in release**:

```rust
let total = 4_000_000_000u32 * 2; // release: wraps to 3_705_032_704
```

**Fix:** `checked_mul`, `saturating_add`, etc. Enable `clippy::arithmetic_side_effects`.

### Deadlocks

Rust prevents data races but NOT deadlocks. Lock ordering bugs, double-locking non-reentrant `std::sync::Mutex`, holding locks across `.await`.

### Memory Leaks

`std::mem::forget` is safe. `Rc`/`Arc` reference cycles leak. Use `Weak` to break cycles.

### Logic Errors / Off-by-One

The compiler can't verify your algorithm. Tests are the only defense.

### RefCell Runtime Panics

`RefCell` moves borrow checking to runtime — violations are panics, not compile errors.

### Clone Abuse

Cloning to satisfy the borrow checker is an antipattern. Question every `.clone()` — can you borrow, move, or restructure?

### 'Arc<Mutex<T>> Performance Trap

One production case: 78% of CPU in atomic operations from excessive `Arc` cloning. Use `Rc` in single-threaded contexts. Write `Arc::clone(&x)` (not `x.clone()`) to make refcount clones visible.

### Path::join with Absolute Paths

```rust
Path::new("/usr").join("/local/bin") // Result: "/local/bin" — base silently dropped!
```

### Forgetting .await

Missing `.await` on a future silently does nothing. No error, no warning in many cases.

### Ignoring Results

`let _ = file.write_all(b"data");` silently discards the error.

---

## Async Pitfalls

### Holding MutexGuard Across .await — THE #1 ASYNC FOOTGUN

```rust
// BAD: deadlock risk
let mut data = state.lock().unwrap();
data.value = expensive_call().await; // lock held across .await!
// GOOD: scope the lock, clone what you need, drop, then .await
```

Use `std::sync::Mutex` when lock is never held across `.await`. Use `tokio::sync::Mutex` only when you must hold across `.await`.

### Blocking the Runtime

`std::thread::sleep()`, CPU-heavy work, or sync I/O inside async = starves other tasks. Use `tokio::task::spawn_blocking`.

### Cancellation Unsafety

Any future can be cancelled at any `.await` point. `tokio::select!` drops the losing branch. Partial writes, incremented counters may be left inconsistent. Cancel-safe: `recv()`, `read()`. NOT cancel-safe: `read_line()`, `write_all()`.

### Dropping JoinHandle Does NOT Cancel a Task

`drop(handle)` — task keeps running! Must call `handle.abort()` or use `CancellationToken`.

### select! Pitfalls

- Recreating futures inside select loop = waste. Hoist outside.
- One always-ready branch starves others.

---

## File/Folder Structure Antipatterns

### God Modules

Everything in `lib.rs`/`main.rs`. Kills readability and recompiles everything on any change.

### Over-Modularizing

One function per file = navigation nightmare. A module should represent a *concept*, not a function.

### mod.rs Sprawl

Dozens of identically-named `mod.rs` files. Use modern `foo.rs` + `foo/` directory style (since Rust 2018).

### Re-Export Chaos

Glob re-exports (`pub use submod::*`) make it impossible to trace where items are defined. Be explicit.

### Circular Module Dependencies

Rust allows cycles within a crate. They create entangled messes that are hard to refactor. Extract shared types to break cycles.

### Monolithic Crates

Cargo parallelizes across crates, not within. One giant crate = slow builds. Use workspaces.

### Overusing `pub`

Everything `pub` defeats the visibility system. Start private, use `pub(crate)` for internal sharing, `pub` only for actual API surface.

---

## Module Organization

- Use `module.rs` + `module/` directory (not `mod.rs`)
- Group by domain (`services/conversation.rs`) not by type (`structs/conversation_service.rs`)
- Re-export key types at module roots with `pub use`, keep submodules private
- Max 3 levels of nesting; deeper = separate crate

### When to Split

- Multiple distinct responsibilities in one file
- Groups of items that don't cross-reference each other
- File exceeds ~500 lines (tests excluded)
- Frequent merge conflicts on different parts of the same file

---

## Function Design

- <50 lines of logic (target), >100 must split
- A comment explaining a code block = a function name wanting to exist
- Nesting >3-4 levels = extract
- >7 params = too much responsibility, or group into a struct

---

## Struct Design

- Fields private by default; provide `new()` + accessors
- **Builder**: 4+ fields with some optional, or construction needs validation
- **Newtype**: type safety, orphan rule bypass, invariant encapsulation
- **Typestate**: compile-time state transition enforcement

---

## Error Handling

- `thiserror` when callers match on variants; `anyhow` when they just propagate/report
- Layer errors per module: `RepositoryError → ServiceError → AppError` via `From`
- Always preserve error chain (`#[source]` or `#[from]`)
- Enrich with context at call sites (`.context("what was happening")`)
- No `unwrap()` except logically impossible failures (with comment) or tests

---

## Trait Design

- Small, focused traits (ISP) — compose with `T: Read + Write`
- Default to generics; use `dyn Trait` for heterogeneous collections or binary size
- Don't create a trait for a single implementation with no foreseeable abstraction need

## Enums vs Traits

| Criterion | Enum | Trait |
| ----------- | ------ | ------- |
| Variant set | Closed, known at compile time | Open, extensible |
| New variant by | Crate author | Anyone |
| Pattern matching | Natural, exhaustive | N/A |
| Performance | No vtable | vtable indirection |

---

## Handler/Service/Repository Layering (Axum)

```txt
Handler (api/handlers/) — THIN: extract, delegate, respond. ~15-20 lines max.
    ↓
Service (services/) — Business logic, orchestration, transaction boundaries
    ↓
Repository (db/) — Data access only, returns domain models, no HTTP knowledge
    ↓
Database
```

- Error conversion at boundaries via `From` impls
- Dependency arrow points inward

---

## Visibility

- Everything starts private (default)
- `pub(super)` — share within module subtree
- `pub(crate)` — internal helpers shared across modules
- `pub` — public API items only
- Enable `unreachable_pub` lint

---

## DI Without Framework

- `Arc<dyn Trait>` for services needing testability (negligible overhead in web apps)
- Generics for hot-path performance-critical code
- Wire in a single bootstrap module
- AppState as DI container is Axum-idiomatic

---

## Testing

- Unit tests: `#[cfg(test)] mod tests` in same file
- Integration tests: `tests/` directory with `tests/common/mod.rs` for shared helpers
- Mock trait implementations for service-layer testing without DB
- `#[sqlx::test]` for repository-layer testing

---

## Clippy Lints to Always Enable

### Recommended Cargo.toml

```toml
[lints.clippy]
pedantic = { level = "warn", priority = -1 }

# Panic prevention
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
indexing_slicing = "warn"

# Debug leftovers
dbg_macro = "warn"
todo = "warn"
print_stdout = "warn"
print_stderr = "warn"

# Casting safety
cast_possible_truncation = "warn"
cast_sign_loss = "warn"
cast_possible_wrap = "warn"
arithmetic_side_effects = "warn"

# Safety docs
undocumented_unsafe_blocks = "warn"

# Clarity
clone_on_ref_ptr = "warn"  # Make Arc::clone() explicit
```

Use `#[cfg(test)] #[allow(...)]` to relax in test code.

---

## Sources

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
- [Effective Rust](https://effective-rust.com/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)
- [Clippy Lint Configuration](https://doc.rust-lang.org/nightly/clippy/lint_configuration.html)
- [Tor Rust Coding Standards](https://tor-code.readthedocs.io/en/latest/HACKING/CodingStandardsRust.html)
- [Pitfalls of Safe Rust — corrode.dev](https://corrode.dev/blog/pitfalls-of-safe-rust/)
- [Sharp Edges in Rust std — corrode.dev](https://corrode.dev/blog/sharp-edges-in-rust-std/)
- [Luca Palmieri — Error Handling](https://lpalmieri.com/posts/error-handling-rust/)
- [howtocodeit — Hexagonal Architecture in Rust](https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust)
- [Hoverbear — State Machine Patterns](https://hoverbear.org/blog/rust-state-machine-pattern/)
- [Cliffle — Typestate Pattern](https://cliffle.com/blog/rust-typestate/)
- [Tokio Tutorial — Shared State](https://tokio.rs/tokio/tutorial/shared-state)
- [Cancelling Async Rust — sunshowers.io](https://sunshowers.io/posts/cancelling-async-rust/)
- [matklad — Notes on Module System](https://matklad.github.io/2021/11/27/notes-on-module-system.html)
- [Dmitry Frank — Rust Module System Encourages Bad Practices](https://dmitryfrank.com/articles/rust_module_system_encourages_bad_practices)
