//! Goal MD file format — the on-disk shape of a goal under
//! `~/.bobe/goals/<id>.md`.
//!
//! Goals are *living documents*: the chat agent reads them on every
//! relevant turn and edits sections via the SDK Edit tool as it
//! learns more about the user's relationship to the goal. This module
//! owns the parser/serializer; consumers (`GoalsService`, the API
//! layer) treat goal files as opaque [`GoalDoc`] values.
//!
//! ## File shape
//!
//! ```md
//! > This is a living document. Update any section as you learn more
//! > about the user's relationship to this goal.
//!
//! # {title}
//!
//! **ID**: <uuid>
//! **Status**: active|paused|completed|archived
//! **Priority**: 0-5
//! **Created**: <iso-8601>
//! **Updated**: <iso-8601>
//!
//! ## Summary
//! ...
//!
//! ## Why It Matters
//! ...
//!
//! ## How They're Working On It
//! ...
//!
//! ## Patterns Observed
//! ...
//!
//! ## Attitude & Feelings
//! ...
//!
//! ## Open Questions
//! ...
//!
//! ## Notes
//! - YYYY-MM-DD — entry
//! ```
//!
//! Sections are optional in the file (a fresh goal may only have
//! Summary populated). Empty sections round-trip as empty strings.

use std::str::FromStr;

use chrono::{DateTime, Utc};

use crate::models::ids::GoalId;
use crate::models::types::GoalStatus;

/// Reminder injected as the first content of every goal file. The
/// chat agent sees this each Read; helps anchor the "you may Edit
/// this" framing without a separate skill block.
pub(crate) const LIVING_DOCUMENT_PREAMBLE: &str = "> This is a living document. Update any section as you learn more\n\
     > about the user's relationship to this goal.\n";

/// Stable section names, in canonical order. The serializer always
/// emits sections in this order (skipping empty ones counts the same
/// as emitting empty headers — we always emit headers so the agent
/// has a stable place to write).
#[cfg(test)]
const SECTIONS: &[&str] = &[
    "Summary",
    "Why It Matters",
    "How They're Working On It",
    "Patterns Observed",
    "Attitude & Feelings",
    "Open Questions",
    "Notes",
];

/// Parsed goal document. Round-trips through [`to_md`] and [`parse`]
/// without losing fields the parser knows about. Unknown sections in
/// a user-edited file are preserved in [`GoalDoc::extra_sections`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalDoc {
    pub(crate) id: GoalId,
    pub(crate) title: String,
    pub(crate) status: GoalStatus,
    /// Priority on 0-5 — 0=low, 5=urgent. Stored as raw integer rather
    /// than the legacy `GoalPriority` enum so the agent can express
    /// finer gradation without the daemon changing types.
    pub(crate) priority: u8,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) summary: String,
    pub(crate) why_it_matters: String,
    pub(crate) how_working_on_it: String,
    pub(crate) patterns_observed: String,
    pub(crate) attitude_feelings: String,
    pub(crate) open_questions: String,
    pub(crate) notes: String,
    /// Any `## Section` headers the parser doesn't know about,
    /// preserved in original order. Lets the agent invent sections
    /// without them being silently stripped on the next round-trip.
    pub(crate) extra_sections: Vec<(String, String)>,
}

impl GoalDoc {
    /// Build a fresh doc with a new id, current timestamps, just the
    /// title + summary populated. Used when the chat agent or the API
    /// proposes a brand-new goal.
    pub(crate) fn new(title: impl Into<String>, summary: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: GoalId::new(),
            title: title.into(),
            status: GoalStatus::Active,
            priority: 2,
            created_at: now,
            updated_at: now,
            summary: summary.into(),
            why_it_matters: String::new(),
            how_working_on_it: String::new(),
            patterns_observed: String::new(),
            attitude_feelings: String::new(),
            open_questions: String::new(),
            notes: String::new(),
            extra_sections: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum GoalParseError {
    #[error("missing required ID header")]
    MissingId,
    #[error("invalid ID: {0}")]
    InvalidId(String),
    #[error("missing title (# heading)")]
    MissingTitle,
    #[error("invalid status: {0}")]
    InvalidStatus(String),
    #[error("invalid priority: {0}")]
    InvalidPriority(String),
    #[error("invalid timestamp '{field}': {value}")]
    InvalidTimestamp { field: &'static str, value: String },
}

/// Serialize a [`GoalDoc`] to MD. Always emits the living-document
/// preamble, the title heading, the metadata header, then every
/// canonical section (empty ones get a header with no body), then
/// any extra sections in their original order.
pub(crate) fn to_md(doc: &GoalDoc) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(2048);
    out.push_str(LIVING_DOCUMENT_PREAMBLE);
    out.push('\n');
    let _ = writeln!(out, "# {}\n", doc.title.trim());
    let _ = writeln!(out, "**ID**: {}", doc.id);
    let _ = writeln!(out, "**Status**: {}", doc.status.as_str());
    let _ = writeln!(out, "**Priority**: {}", doc.priority);
    let _ = writeln!(
        out,
        "**Created**: {}",
        doc.created_at.format("%Y-%m-%dT%H:%M:%SZ")
    );
    let _ = writeln!(
        out,
        "**Updated**: {}",
        doc.updated_at.format("%Y-%m-%dT%H:%M:%SZ")
    );

    for (name, body) in canonical_sections(doc) {
        out.push('\n');
        let _ = writeln!(out, "## {name}");
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            out.push('\n');
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    for (name, body) in &doc.extra_sections {
        out.push('\n');
        let _ = writeln!(out, "## {name}");
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            out.push('\n');
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    out
}

fn canonical_sections(doc: &GoalDoc) -> [(&'static str, &str); 7] {
    [
        ("Summary", doc.summary.as_str()),
        ("Why It Matters", doc.why_it_matters.as_str()),
        ("How They're Working On It", doc.how_working_on_it.as_str()),
        ("Patterns Observed", doc.patterns_observed.as_str()),
        ("Attitude & Feelings", doc.attitude_feelings.as_str()),
        ("Open Questions", doc.open_questions.as_str()),
        ("Notes", doc.notes.as_str()),
    ]
}

/// Parse a goal MD file. Tolerates extra whitespace, missing optional
/// sections, and section names in any order. Required: title (`# `
/// heading) and `**ID**:` line.
pub(crate) fn parse(input: &str) -> Result<GoalDoc, GoalParseError> {
    let mut title: Option<String> = None;
    let mut id: Option<GoalId> = None;
    let mut status = GoalStatus::Active;
    let mut priority: u8 = 2;
    let mut created_at: Option<DateTime<Utc>> = None;
    let mut updated_at: Option<DateTime<Utc>> = None;

    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;

    for raw_line in input.lines() {
        let line = raw_line.trim_end();

        if let Some(rest) = line.strip_prefix("# ") {
            // Title
            if title.is_none() {
                title = Some(rest.trim().to_string());
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("## ") {
            // Section start: flush the previous section, begin a new one.
            if let Some((name, body)) = current.take() {
                sections.push((name, body));
            }
            current = Some((rest.trim().to_string(), String::new()));
            continue;
        }

        // Metadata header lines (only honored before we hit the first `##`).
        if current.is_none()
            && let Some((key, value)) = parse_metadata_line(line)
        {
            match key {
                "ID" => {
                    id = Some(
                        GoalId::from_str(value)
                            .map_err(|e| GoalParseError::InvalidId(format!("{value}: {e}")))?,
                    );
                }
                "Status" => {
                    status = parse_status(value)?;
                }
                "Priority" => {
                    priority = value
                        .parse::<u8>()
                        .map_err(|_| GoalParseError::InvalidPriority(value.into()))?;
                }
                "Created" => {
                    created_at = Some(parse_iso_8601(value, "Created")?);
                }
                "Updated" => {
                    updated_at = Some(parse_iso_8601(value, "Updated")?);
                }
                _ => {} // Unknown metadata keys are ignored.
            }
            continue;
        }

        // Body content for the current section, if any.
        if let Some((_, ref mut body)) = current {
            body.push_str(line);
            body.push('\n');
        }
    }

    if let Some((name, body)) = current {
        sections.push((name, body));
    }

    let id = id.ok_or(GoalParseError::MissingId)?;
    let title = title.ok_or(GoalParseError::MissingTitle)?;
    let now = Utc::now();
    let mut doc = GoalDoc {
        id,
        title,
        status,
        priority,
        created_at: created_at.unwrap_or(now),
        updated_at: updated_at.unwrap_or(now),
        summary: String::new(),
        why_it_matters: String::new(),
        how_working_on_it: String::new(),
        patterns_observed: String::new(),
        attitude_feelings: String::new(),
        open_questions: String::new(),
        notes: String::new(),
        extra_sections: Vec::new(),
    };

    for (name, body) in sections {
        let trimmed = body.trim().to_string();
        match name.as_str() {
            "Summary" => doc.summary = trimmed,
            "Why It Matters" => doc.why_it_matters = trimmed,
            "How They're Working On It" => doc.how_working_on_it = trimmed,
            "Patterns Observed" => doc.patterns_observed = trimmed,
            "Attitude & Feelings" => doc.attitude_feelings = trimmed,
            "Open Questions" => doc.open_questions = trimmed,
            "Notes" => doc.notes = trimmed,
            _ => doc.extra_sections.push((name, trimmed)),
        }
    }

    Ok(doc)
}

/// `**Key**: value` parser tolerating whitespace, returning the
/// trimmed value if the line matches the pattern.
fn parse_metadata_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_start();
    let rest = line.strip_prefix("**")?;
    let close = rest.find("**:")?;
    let key = &rest[..close];
    let value = rest[close + "**:".len()..].trim();
    Some((key, value))
}

fn parse_status(value: &str) -> Result<GoalStatus, GoalParseError> {
    match value.to_lowercase().as_str() {
        "active" => Ok(GoalStatus::Active),
        "paused" => Ok(GoalStatus::Paused),
        "completed" => Ok(GoalStatus::Completed),
        "archived" => Ok(GoalStatus::Archived),
        other => Err(GoalParseError::InvalidStatus(other.to_string())),
    }
}

fn parse_iso_8601(value: &str, field: &'static str) -> Result<DateTime<Utc>, GoalParseError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| GoalParseError::InvalidTimestamp {
            field,
            value: value.into(),
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    fn fresh_doc() -> GoalDoc {
        let mut d = GoalDoc::new("Learn Spanish", "Reach B1 conversational level.");
        d.why_it_matters = "Belonging at partner's family gatherings.".into();
        d.how_working_on_it = "- Duolingo daily\n- iTalki weekly".into();
        d.notes = "- 2026-05-01 — Started Duolingo streak".into();
        d
    }

    #[test]
    fn round_trip_preserves_fields() {
        let original = fresh_doc();
        let md = to_md(&original);
        let parsed = parse(&md).unwrap();
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.title, original.title);
        assert_eq!(parsed.summary, original.summary);
        assert_eq!(parsed.why_it_matters, original.why_it_matters);
        assert_eq!(parsed.how_working_on_it, original.how_working_on_it);
        assert_eq!(parsed.notes, original.notes);
        assert_eq!(parsed.status, original.status);
        assert_eq!(parsed.priority, original.priority);
    }

    #[test]
    fn serialized_includes_living_document_preamble() {
        let md = to_md(&fresh_doc());
        assert!(
            md.starts_with(LIVING_DOCUMENT_PREAMBLE),
            "preamble must be the first content"
        );
    }

    #[test]
    fn serialized_emits_all_canonical_sections() {
        let md = to_md(&GoalDoc::new("X", "Y"));
        for s in SECTIONS {
            assert!(md.contains(&format!("## {s}")), "missing section: {s}");
        }
    }

    #[test]
    fn parse_minimal_goal() {
        let id = GoalId::new();
        let md = format!(
            "# Practice piano\n\
             \n\
             **ID**: {id}\n\
             **Status**: active\n\
             **Priority**: 1\n\
             **Created**: 2026-05-09T10:00:00Z\n\
             **Updated**: 2026-05-09T10:00:00Z\n"
        );
        let doc = parse(&md).unwrap();
        assert_eq!(doc.title, "Practice piano");
        assert_eq!(doc.id, id);
        assert_eq!(doc.priority, 1);
        assert!(doc.summary.is_empty());
    }

    #[test]
    fn parse_preserves_extra_sections() {
        let id = GoalId::new();
        let md = format!(
            "# Goal\n\
             \n\
             **ID**: {id}\n\
             **Status**: active\n\
             **Priority**: 2\n\
             **Created**: 2026-05-09T10:00:00Z\n\
             **Updated**: 2026-05-09T10:00:00Z\n\
             \n\
             ## Summary\n\
             \n\
             A thing.\n\
             \n\
             ## Custom Section\n\
             \n\
             User added this.\n"
        );
        let doc = parse(&md).unwrap();
        assert_eq!(doc.summary, "A thing.");
        assert_eq!(doc.extra_sections.len(), 1);
        assert_eq!(doc.extra_sections[0].0, "Custom Section");
        assert!(doc.extra_sections[0].1.contains("User added this"));
    }

    #[test]
    fn parse_rejects_missing_id() {
        let md = "# Goal\n\n**Status**: active\n";
        assert!(matches!(parse(md), Err(GoalParseError::MissingId)));
    }

    #[test]
    fn parse_rejects_missing_title() {
        let md = format!("**ID**: {}\n**Status**: active\n", GoalId::new());
        assert!(matches!(parse(&md), Err(GoalParseError::MissingTitle)));
    }

    #[test]
    fn parse_tolerates_unknown_metadata_keys() {
        let id = GoalId::new();
        let md = format!(
            "# Goal\n\n**ID**: {id}\n**Status**: active\n**Priority**: 2\n\
             **Created**: 2026-05-09T10:00:00Z\n**Updated**: 2026-05-09T10:00:00Z\n\
             **NewKey**: future-field\n"
        );
        // Should parse without error.
        let doc = parse(&md).unwrap();
        assert_eq!(doc.id, id);
    }

    #[test]
    fn parse_status_variants() {
        assert_eq!(parse_status("active").unwrap(), GoalStatus::Active);
        assert_eq!(parse_status("PAUSED").unwrap(), GoalStatus::Paused);
        assert_eq!(parse_status("Completed").unwrap(), GoalStatus::Completed);
        assert_eq!(parse_status("archived").unwrap(), GoalStatus::Archived);
        assert!(parse_status("frobbed").is_err());
    }
}
