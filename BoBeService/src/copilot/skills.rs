//! Baked-in skill definitions for Copilot worker classes.
//!
//! Each `SKILL.md` is the stable per-class identity loaded via
//! `SessionConfig::skill_directories`. Per-job `instructions` carry the
//! turn-specific brief; the skill carries the agent's persona, output
//! contract, and access patterns (which files to read).
//!
//! The CLI loads `<skill_dir>/SKILL.md` as system context for every
//! turn. We write these on daemon start so a freshly-installed BoBe
//! ships with the same skill the daemon was built against.

use std::path::Path;

use tracing::{debug, warn};

use super::types::WorkerClass;

/// `SKILL.md` for `WorkerClass::Decide`. The decision engine is invoked
/// every time a proactive trigger fires (screen capture, goal, check-in)
/// — its output gates whether the chat agent wakes up and produces
/// user-visible output. Its job is to be fast, conservative, and
/// strictly structured.
const DECIDE_SKILL_MD: &str = r#"# Decide Skill

You are BoBe's engagement decision engine. You answer one question per
turn: should BoBe reach out to the user proactively right now?

Your output is consumed by the daemon — not shown to the user. Reply
with **a single JSON object only**, no preamble, no code fences:

```
{"job_id":"<echo>","output":{"decision":"reach_out|idle|need_more_info","reasoning":"<1-2 sentences>"}}
```

Decision values:

- `reach_out` — the user appears stuck, has just hit a relevant goal
  milestone, or could clearly benefit from BoBe's voice now. The user
  is *not* in deep focus on something unrelated.
- `idle` — leave the user alone. They are focused, they have not
  asked for help, the trigger is noise, or BoBe just spoke recently.
  **This is the default.** When uncertain, choose `idle`.
- `need_more_info` — the trigger is genuinely ambiguous AND another
  capture in a few minutes would clarify it. Use sparingly; prefer
  `idle` over speculation.

## Inputs every turn

Each user turn carries (in the `input` field):

- `trigger_kind` — `capture` (screen change), `goal` (goal-related
  trigger), or other.
- `current_activity` — short text summary of what's on the user's
  screen right now (or the goal text for goal triggers).
- `recent_ai_messages` — the last few messages BoBe sent the user.
  Used to avoid spamming.
- `current_time` — local wall-clock time (string).

## Context already loaded

You have memory.md auto-injected at session start (the user's persona,
recent themes, BoBe's notes about them). Treat it as ground truth for
preferences and current life context.

You also have access to `~/.bobe/goals/` — each `<id>.md` file is one
active goal with frontmatter (`id`, `status`, `priority`, `created_at`)
and the goal body. **Use the Read tool on this directory whenever the
trigger could plausibly relate to a goal.** A `reach_out` decision tied
to a goal is much higher value than one that doesn't.

## Heuristics

- The user is in flow (long uninterrupted activity in the same app)
  → `idle`.
- A recent BoBe message is in `recent_ai_messages` and the user
  hasn't replied → `idle`. Don't double-tap.
- The trigger maps cleanly to an active goal in
  `~/.bobe/goals/` AND the user looks stuck (e.g., scrolling docs for
  something the goal already addressed) → `reach_out`.
- Off-hours (very late night, very early morning) → bias hard toward
  `idle` unless the trigger is itself an off-hours goal milestone.
- Brand-new context with no goal correspondence and no clear stuck
  signal → `idle`.

Your reasoning field stays short — it's for daemon logs, not user
display. Do not include any text outside the JSON object.
"#;

/// Write skill files into `<data_dir>/skills/<class>/SKILL.md` if they
/// don't exist yet. Idempotent: never overwrites an existing file —
/// the user (or a future migration) can edit a class's skill in place.
///
/// Failures are logged and swallowed: the worker session falls back to
/// running without skill context, which is degraded but not fatal.
pub(crate) async fn ensure_skills(data_dir: &Path) {
    for &(class, content) in &[(WorkerClass::Decide, DECIDE_SKILL_MD)] {
        let dir = data_dir.join("skills").join(class.name());
        let path = dir.join("SKILL.md");

        if path.exists() {
            debug!(
                class = %class.name(),
                path = %path.display(),
                "skill exists, leaving as-is"
            );
            continue;
        }

        if let Err(e) = tokio::fs::create_dir_all(&dir).await {
            warn!(
                class = %class.name(),
                err = %e,
                "could not create skill dir"
            );
            continue;
        }
        if let Err(e) = tokio::fs::write(&path, content).await {
            warn!(
                class = %class.name(),
                err = %e,
                "could not write SKILL.md"
            );
            continue;
        }
        tracing::info!(
            class = %class.name(),
            path = %path.display(),
            "wrote default SKILL.md"
        );
    }
}
