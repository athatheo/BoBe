//! One-time migration from the last published SQL narrative stores to files.

use std::fmt::Write as _;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::models::ids::GoalId;
use crate::models::types::GoalStatus;
use crate::services::goals::goal_md::{GoalDoc, parse, to_md};

const VERSION: i64 = 1;

#[derive(Serialize)]
struct Backup {
    version: i64,
    goals: Vec<LegacyGoal>,
    plans: Vec<LegacyPlan>,
    plan_steps: Vec<LegacyPlanStep>,
    memories: Vec<LegacyMemory>,
    observations: Vec<LegacyObservation>,
}

#[derive(Serialize)]
struct LegacyGoal {
    id: Vec<u8>,
    content: String,
    priority: String,
    source: String,
    status: String,
    enabled: i64,
    inference_reason: Option<String>,
    embedding: Option<String>,
    created_at: String,
    updated_at: String,
}
#[derive(Serialize)]
struct LegacyPlan {
    id: Vec<u8>,
    goal_id: Vec<u8>,
    summary: String,
    status: String,
    failure_count: i64,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
}
#[derive(Serialize)]
struct LegacyPlanStep {
    id: Vec<u8>,
    plan_id: Vec<u8>,
    step_order: i64,
    content: String,
    status: String,
    result: Option<String>,
    error: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
    created_at: String,
}
#[derive(Serialize)]
struct LegacyMemory {
    id: Vec<u8>,
    content: String,
    memory_type: String,
    enabled: i64,
    category: String,
    source: String,
    created_at: String,
    updated_at: String,
}
#[derive(Serialize)]
struct LegacyObservation {
    id: Vec<u8>,
    source: String,
    content: String,
    category: String,
    metadata: Option<String>,
    created_at: String,
    updated_at: String,
}

pub(crate) async fn run(pool: &SqlitePool, data_root: &Path) -> Result<(), AppError> {
    ensure_history(pool).await?;
    if applied(pool).await? || !has_legacy_tables(pool).await? {
        return Ok(());
    }

    let backup = load_backup(pool).await?;
    let backup_path = data_root.join("migration-backups/legacy-narrative-v1.json");
    crate::util::durable_fs::atomic_write(&backup_path, &serde_json::to_vec_pretty(&backup)?)
        .await?;
    verify_backup(&backup_path, &backup).await?;

    write_goals(data_root, &backup).await?;
    write_memory(data_root, &backup).await?;
    verify_files(data_root, &backup).await?;

    let mut transaction = pool.begin().await?;
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS goal_plan_steps;
         DROP TABLE IF EXISTS goal_plans;
         DROP TABLE IF EXISTS goals;
         DROP TABLE IF EXISTS memories;
         DROP TABLE IF EXISTS observations;",
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query("INSERT INTO bobe_migrations (version) VALUES (?1)")
        .bind(VERSION)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    tracing::info!(version = VERSION, "database.legacy_narrative_migrated");
    Ok(())
}

async fn ensure_history(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::query("CREATE TABLE IF NOT EXISTS bobe_migrations (version INTEGER PRIMARY KEY NOT NULL, applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')))")
        .execute(pool).await?;
    Ok(())
}
async fn applied(pool: &SqlitePool) -> Result<bool, AppError> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM bobe_migrations WHERE version = ?1")
            .bind(VERSION)
            .fetch_one(pool)
            .await?
            != 0,
    )
}
async fn has_legacy_tables(pool: &SqlitePool) -> Result<bool, AppError> {
    for name in [
        "goals",
        "goal_plans",
        "goal_plan_steps",
        "memories",
        "observations",
    ] {
        if table_exists(pool, name).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn table_exists(pool: &SqlitePool, name: &str) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
    )
    .bind(name)
    .fetch_one(pool)
    .await?
        != 0)
}

async fn load_backup(pool: &SqlitePool) -> Result<Backup, AppError> {
    let goals = if table_exists(pool, "goals").await? {
        sqlx::query("SELECT id, content, priority, source, status, enabled, inference_reason, embedding, created_at, updated_at FROM goals ORDER BY created_at").fetch_all(pool).await?.into_iter().map(|r| LegacyGoal { id:r.get(0), content:r.get(1), priority:r.get(2), source:r.get(3), status:r.get(4), enabled:r.get(5), inference_reason:r.get(6), embedding:r.get(7), created_at:r.get(8), updated_at:r.get(9) }).collect()
    } else {
        Vec::new()
    };
    let plans = if table_exists(pool, "goal_plans").await? {
        sqlx::query("SELECT id, goal_id, summary, status, failure_count, last_error, created_at, updated_at FROM goal_plans ORDER BY created_at").fetch_all(pool).await?.into_iter().map(|r| LegacyPlan { id:r.get(0), goal_id:r.get(1), summary:r.get(2), status:r.get(3), failure_count:r.get(4), last_error:r.get(5), created_at:r.get(6), updated_at:r.get(7) }).collect()
    } else {
        Vec::new()
    };
    let plan_steps = if table_exists(pool, "goal_plan_steps").await? {
        sqlx::query("SELECT id, plan_id, step_order, content, status, result, error, started_at, completed_at, created_at FROM goal_plan_steps ORDER BY plan_id, step_order").fetch_all(pool).await?.into_iter().map(|r| LegacyPlanStep { id:r.get(0), plan_id:r.get(1), step_order:r.get(2), content:r.get(3), status:r.get(4), result:r.get(5), error:r.get(6), started_at:r.get(7), completed_at:r.get(8), created_at:r.get(9) }).collect()
    } else {
        Vec::new()
    };
    let memories = if table_exists(pool, "memories").await? {
        sqlx::query("SELECT id, content, memory_type, enabled, category, source, created_at, updated_at FROM memories ORDER BY created_at").fetch_all(pool).await?.into_iter().map(|r| LegacyMemory { id:r.get(0), content:r.get(1), memory_type:r.get(2), enabled:r.get(3), category:r.get(4), source:r.get(5), created_at:r.get(6), updated_at:r.get(7) }).collect()
    } else {
        Vec::new()
    };
    let observations = if table_exists(pool, "observations").await? {
        sqlx::query("SELECT id, source, content, category, metadata, created_at, updated_at FROM observations ORDER BY created_at").fetch_all(pool).await?.into_iter().map(|r| LegacyObservation { id:r.get(0), source:r.get(1), content:r.get(2), category:r.get(3), metadata:r.get(4), created_at:r.get(5), updated_at:r.get(6) }).collect()
    } else {
        Vec::new()
    };
    Ok(Backup {
        version: VERSION,
        goals,
        plans,
        plan_steps,
        memories,
        observations,
    })
}

async fn write_goals(root: &Path, backup: &Backup) -> Result<(), AppError> {
    let directory = root.join("goals");
    for goal in &backup.goals {
        let uuid = uuid::Uuid::from_slice(&goal.id)
            .map_err(|error| AppError::Internal(format!("legacy goal id: {error}")))?;
        let id = GoalId::from(uuid);
        let path = directory.join(format!("{id}.md"));
        if path.exists() {
            continue;
        }
        let plans: Vec<&LegacyPlan> = backup
            .plans
            .iter()
            .filter(|plan| plan.goal_id == goal.id)
            .collect();
        let plan_notes = plans
            .iter()
            .map(|plan| {
                let steps = backup
                    .plan_steps
                    .iter()
                    .filter(|step| step.plan_id == plan.id)
                    .map(|step| {
                        format!(
                            "- [{}] {}{}",
                            step.status,
                            step.content,
                            step.result
                                .as_ref()
                                .map(|r| format!(" — {r}"))
                                .unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "### Legacy plan ({})\n{}\n{}",
                    plan.status, plan.summary, steps
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let mut doc = GoalDoc {
            id,
            title: goal.content.clone(),
            status: status(&goal.status, goal.enabled),
            priority: priority(&goal.priority),
            created_at: timestamp(&goal.created_at)?,
            updated_at: timestamp(&goal.updated_at)?,
            summary: goal.inference_reason.clone().unwrap_or_default(),
            why_it_matters: String::new(),
            how_working_on_it: plan_notes,
            patterns_observed: String::new(),
            attitude_feelings: String::new(),
            open_questions: String::new(),
            notes: format!("Migrated from legacy source: {}", goal.source),
            extra_sections: Vec::new(),
        };
        if doc.title.trim().is_empty() {
            doc.title = "Migrated goal".into();
        }
        crate::util::durable_fs::atomic_write(&path, to_md(&doc).as_bytes()).await?;
    }
    Ok(())
}

async fn write_memory(root: &Path, backup: &Backup) -> Result<(), AppError> {
    let path = root.join("memory.md");
    let existing = tokio::fs::read_to_string(&path).await.unwrap_or_else(|_| {
        "# BoBe Memory\n\n## Profile\n\n## Active Goals\n\n## Long-term\n\n## Recent\n".into()
    });
    let marker = "## Migrated Legacy Data";
    if existing.contains(marker) {
        return Ok(());
    }
    let mut migrated = String::from("\n\n## Migrated Legacy Data\n");
    for memory in &backup.memories {
        let _ = write!(
            migrated,
            "\n- [{} / {} / {}] {}",
            memory.created_at,
            memory.memory_type,
            memory.category,
            memory.content.trim()
        );
    }
    for observation in &backup.observations {
        let _ = write!(
            migrated,
            "\n- [{} / observation / {} / {}] {}",
            observation.created_at,
            observation.source,
            observation.category,
            observation.content.trim()
        );
    }
    migrated.push('\n');
    crate::util::durable_fs::atomic_write(
        &path,
        format!("{}{}", existing.trim_end(), migrated).as_bytes(),
    )
    .await
}

async fn verify_backup(path: &Path, expected: &Backup) -> Result<(), AppError> {
    let value: serde_json::Value = serde_json::from_slice(&tokio::fs::read(path).await?)?;
    for (name, count) in [
        ("goals", expected.goals.len()),
        ("plans", expected.plans.len()),
        ("plan_steps", expected.plan_steps.len()),
        ("memories", expected.memories.len()),
        ("observations", expected.observations.len()),
    ] {
        if value[name].as_array().map(Vec::len) != Some(count) {
            return Err(AppError::Internal(format!(
                "legacy migration backup verification failed for {name}"
            )));
        }
    }
    Ok(())
}
async fn verify_files(root: &Path, expected: &Backup) -> Result<(), AppError> {
    for goal in &expected.goals {
        let id = GoalId::from(
            uuid::Uuid::from_slice(&goal.id).map_err(|e| AppError::Internal(e.to_string()))?,
        );
        let body = tokio::fs::read_to_string(root.join("goals").join(format!("{id}.md"))).await?;
        if parse(&body)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .id
            != id
        {
            return Err(AppError::Internal(
                "migrated goal verification failed".into(),
            ));
        }
    }
    let memory = tokio::fs::read_to_string(root.join("memory.md")).await?;
    if !memory.contains("## Migrated Legacy Data") {
        return Err(AppError::Internal(
            "migrated memory verification failed".into(),
        ));
    }
    Ok(())
}
fn status(value: &str, enabled: i64) -> GoalStatus {
    if enabled == 0 {
        GoalStatus::Archived
    } else {
        match value {
            "paused" => GoalStatus::Paused,
            "completed" => GoalStatus::Completed,
            "archived" => GoalStatus::Archived,
            _ => GoalStatus::Active,
        }
    }
}
fn priority(value: &str) -> u8 {
    match value {
        "urgent" => 5,
        "high" => 4,
        "low" => 1,
        _ => 2,
    }
}
fn timestamp(value: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|e| AppError::Internal(format!("legacy timestamp '{value}': {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn migrates_verifies_drops_and_is_idempotent() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/legacy_narrative_fixture.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let root = std::env::temp_dir().join(format!("bobe-migration-{}", uuid::Uuid::new_v4()));
        run(&pool, &root).await.unwrap();
        run(&pool, &root).await.unwrap();
        assert!(
            root.join("migration-backups/legacy-narrative-v1.json")
                .exists()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sqlite_master WHERE name='goals'")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        let goals = tokio::fs::read_dir(root.join("goals")).await.unwrap();
        drop(goals);
        let memory = tokio::fs::read_to_string(root.join("memory.md"))
            .await
            .unwrap();
        assert!(memory.contains("remember this"));
        assert!(memory.contains("observed this"));
        let _ignored = tokio::fs::remove_dir_all(root).await;
    }
}
