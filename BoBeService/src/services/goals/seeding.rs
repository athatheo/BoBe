use tracing::{debug, info, warn};

use crate::error::AppError;

use super::goal_md::GoalDoc;
use super::goals_service::GoalsService;

#[derive(Debug)]
pub(crate) struct SeedResult {
    pub(crate) created: u32,
    pub(crate) skipped: u32,
    pub(crate) errors: u32,
}

/// First-run only: drops a starter goal so the UI isn't empty on first launch.
pub(crate) async fn seed_sample_goal(
    goals_service: &GoalsService,
) -> Result<SeedResult, AppError> {
    let mut result = SeedResult { created: 0, skipped: 0, errors: 0 };

    match goals_service.list_all().await {
        Ok(existing) if !existing.is_empty() => {
            debug!(count = existing.len(), "goals_seeding.skipped_existing");
            result.skipped += 1;
            return Ok(result);
        }
        Ok(_) => {}
        Err(e) => {
            warn!(error = %e, "goals_seeding.list_failed");
            result.errors += 1;
            return Ok(result);
        }
    }

    let mut doc = GoalDoc::new(
        "Getting started with BoBe",
        "A starter goal so you can see how the goals system works. Edit any section, talk to BoBe about it, or delete this and write your own.",
    );
    doc.priority = 1;
    doc.why_it_matters = "Goals are how BoBe tracks what you actually care about across conversations. \
They're plain Markdown files under ~/.bobe/goals/, so you can edit them in any editor and BoBe will pick up the changes."
        .to_string();
    doc.how_working_on_it =
        "Try editing this goal's sections in the Settings panel, or just chat with BoBe about something you want to work on."
            .to_string();

    match goals_service.create(doc).await {
        Ok(saved) => {
            info!(goal_id = %saved.id, "goals_seeding.created");
            result.created += 1;
        }
        Err(e) => {
            warn!(error = %e, "goals_seeding.create_failed");
            result.errors += 1;
        }
    }

    Ok(result)
}
