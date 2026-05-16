use std::path::Path;

use tracing::{debug, warn};

use super::types::WorkerClass;

const DECIDE_SKILL_MD: &str = include_str!("skills/decide.md");
const CHAT_SKILL_MD: &str = include_str!("skills/chat.md");

const SHIPPED_SKILLS: &[(WorkerClass, &str)] = &[
    (WorkerClass::Decide, DECIDE_SKILL_MD),
    (WorkerClass::Chat, CHAT_SKILL_MD),
];

/// Idempotent: never overwrites existing SKILL.md so users can edit in place.
pub(crate) async fn ensure_skills(data_dir: &Path) {
    for &(class, content) in SHIPPED_SKILLS {
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
