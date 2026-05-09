//! Screen-capture learner: ask `VisionWorker` what's on the screen,
//! append a one-liner under `## Recent` in `memory.md`. The nightly
//! Consolidate worker prunes that section.

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::copilot::memory_file::MemoryFile;
use crate::copilot::registry::WorkerRegistry;
use crate::copilot::types::ChatAttachment;
use crate::error::AppError;
use crate::util::text::truncate_str;

/// Section heading inside `memory.md` where capture descriptions land.
/// Matches the section the consolidation prompt is told to prune.
const RECENT_SECTION: &str = "Recent";

/// Soft cap on the description we hand to the agent. Anything longer
/// just bloats memory.md before consolidation runs.
const DESCRIPTION_MAX_LEN: usize = 280;

const VISION_QUESTION: &str = "Describe in one sentence what the user is doing on screen right now. \
     Focus on the activity (writing code in X, reading docs about Y, drafting email about Z), \
     not the visual details. If the screen is mostly empty or system UI, say so plainly.";

pub(crate) struct CaptureLearner {
    workers: Arc<WorkerRegistry>,
    memory_file: Arc<MemoryFile>,
}

impl CaptureLearner {
    pub(crate) fn new(workers: Arc<WorkerRegistry>, memory_file: Arc<MemoryFile>) -> Self {
        Self {
            workers,
            memory_file,
        }
    }

    /// Vision-describe the screenshot, append the description to
    /// memory.md, return it. The returned string is what the trigger
    /// hands to `DecisionEngine` as `context_text`.
    ///
    /// Return-value semantics:
    /// - `Ok(non_empty)` — vision produced a usable description; the
    ///   trigger may proceed to the decision step.
    /// - `Ok("")` — vision produced an empty description (the screen
    ///   was uninformative). Treat as a no-op cycle.
    /// - `Err(_)` — vision worker failed. Trigger should back off
    ///   instead of retrying immediately. This used to be swallowed
    ///   into `Ok("")`, which spun forever when the SDK was
    ///   unavailable; surfacing the error lets the trigger debounce.
    pub(crate) async fn learn(
        &self,
        screenshot: Vec<u8>,
        active_window: Option<&str>,
    ) -> Result<String, AppError> {
        info!(
            window = ?active_window,
            img_kb = screenshot.len() / 1024,
            "capture_learner.started"
        );

        let worker = self.workers.vision().await?;
        let attachment = ChatAttachment::ImageBytes {
            bytes: screenshot,
            mime_type: "image/png",
        };
        let answer = worker
            .analyze(VISION_QUESTION, attachment)
            .await
            .map_err(|e| AppError::Capture(format!("vision worker failed: {e}")))?;

        let description = answer.text.trim();
        if description.is_empty() {
            debug!("capture_learner.empty_description");
            return Ok(String::new());
        }

        let entry = format_entry(description, active_window);
        if let Err(e) = self.memory_file.append_under(RECENT_SECTION, &entry).await {
            warn!(error = %e, "capture_learner.memory_append_failed");
            // Description is still valuable for the decision-engine
            // call even if persistence failed.
        }

        info!(
            chars = description.len(),
            output_tokens = ?answer.output_tokens,
            "capture_learner.appended_to_memory"
        );

        Ok(truncate_str(description, DESCRIPTION_MAX_LEN).to_string())
    }
}

fn format_entry(description: &str, active_window: Option<&str>) -> String {
    let prefix = active_window.map(|w| format!("[{w}] ")).unwrap_or_default();
    format!("{prefix}{}", truncate_str(description, DESCRIPTION_MAX_LEN))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_entry_with_window() {
        let s = format_entry("Coding in Rust", Some("VS Code"));
        assert_eq!(s, "[VS Code] Coding in Rust");
    }

    #[test]
    fn format_entry_without_window() {
        let s = format_entry("Reading docs", None);
        assert_eq!(s, "Reading docs");
    }

    #[test]
    fn format_entry_truncates() {
        let long = "x".repeat(500);
        let s = format_entry(&long, None);
        assert!(s.len() <= DESCRIPTION_MAX_LEN);
    }
}
