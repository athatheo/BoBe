//! Parser for GOALS.md file format.
//!
//! Expected format:
//! ```markdown
//! # My Goals
//!
//! ## High Priority
//! - [ ] Learn TypeScript generics
//! - [x] Set up dev environment
//!
//! ## Medium Priority
//! - [ ] Build a side project
//!
//! ## Low Priority
//! - [ ] Organize bookmarks
//!
//! ## Completed
//! - [x] Configure ESLint
//!
//! ---
//! ## Inferred by BoBe
//! - [ ] Improve Python async skills
//!   > Noticed you debugging asyncio issues frequently (2026-02-01)
//! ```

use std::sync::OnceLock;

use regex::Regex;

fn checkbox_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^-\s*\[([ xX])\]\s*(.+)$").expect("valid checkbox regex"))
}

/// A goal parsed from GOALS.md.
#[derive(Debug, Clone)]
pub struct ParsedGoal {
    pub content: String,
    /// "high" | "medium" | "low"
    pub priority: String,
    pub completed: bool,
    pub is_inferred: bool,
}

/// Parse GOALS.md content into structured goals.
pub fn parse_goals_file(content: &str) -> Vec<ParsedGoal> {
    let mut goals = Vec::new();
    let mut current_priority = "medium".to_owned();
    let mut in_inferred_section = false;
    let mut in_completed_section = false;

    let checkbox_re = checkbox_regex();

    for line in content.lines() {
        let line = line.trim();

        // Track section headers
        if let Some(section_name) = line.strip_prefix("## ") {
            let lower = section_name.trim().to_lowercase();
            if lower.contains("inferred") || lower.contains("bobe") {
                in_inferred_section = true;
                in_completed_section = false;
            } else if lower.contains("high") {
                current_priority = "high".to_owned();
                in_inferred_section = false;
                in_completed_section = false;
            } else if lower.contains("medium") {
                current_priority = "medium".to_owned();
                in_inferred_section = false;
                in_completed_section = false;
            } else if lower.contains("low") {
                current_priority = "low".to_owned();
                in_inferred_section = false;
                in_completed_section = false;
            } else if lower.contains("completed") {
                in_completed_section = true;
                in_inferred_section = false;
            }
            continue;
        }

        // Skip non-checkbox lines
        let Some(caps) = checkbox_re.captures(line) else {
            continue;
        };

        let checkbox = caps.get(1).map(|m| m.as_str()).unwrap_or(" ");
        let goal_content = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
        if goal_content.is_empty() {
            continue;
        }

        let is_completed = checkbox.eq_ignore_ascii_case("x") || in_completed_section;
        let priority = if in_completed_section {
            "medium".to_owned()
        } else {
            current_priority.clone()
        };

        goals.push(ParsedGoal {
            content: goal_content.to_owned(),
            priority,
            completed: is_completed,
            is_inferred: in_inferred_section,
        });
    }

    goals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_goals() {
        let content = r#"# My Goals

## High Priority
- [ ] Learn Rust
- [x] Set up dev environment

## Medium Priority
- [ ] Build a side project

## Low Priority
- [ ] Organize bookmarks

## Completed
- [x] Configure ESLint

---
## Inferred by BoBe

- [ ] Improve async skills
"#;
        let goals = parse_goals_file(content);
        assert_eq!(goals.len(), 6);

        assert_eq!(goals[0].content, "Learn Rust");
        assert_eq!(goals[0].priority, "high");
        assert!(!goals[0].completed);
        assert!(!goals[0].is_inferred);

        assert_eq!(goals[1].content, "Set up dev environment");
        assert!(goals[1].completed);

        assert_eq!(goals[2].priority, "medium");
        assert_eq!(goals[3].priority, "low");

        assert_eq!(goals[4].content, "Configure ESLint");
        assert!(goals[4].completed);

        assert_eq!(goals[5].content, "Improve async skills");
        assert!(goals[5].is_inferred);
    }

    #[test]
    fn parse_empty_content() {
        let goals = parse_goals_file("");
        assert!(goals.is_empty());
    }
}
