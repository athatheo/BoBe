use tracing::{debug, info, warn};

use crate::domain::soul::Soul;
use crate::domain::user_profile::UserProfile;
use crate::error::AppError;
use crate::ports::repos::soul_repo::SoulRepository;
use crate::ports::repos::user_profile_repo::UserProfileRepository;

/// Default souls to seed from embedded assets.
const DEFAULT_SOULS: &[(&str, &str)] = &[("SOUL.md", "default_soul")];

/// Default user profiles to seed from embedded assets.
const DEFAULT_USER_PROFILES: &[(&str, &str)] = &[("USER_PROFILE.md", "default")];

/// Embedded default assets
const DEFAULT_SOUL_MD: &str = include_str!("../assets/defaults/SOUL.md");
const DEFAULT_USER_PROFILE_MD: &str = include_str!("../assets/defaults/USER_PROFILE.md");
const DEFAULT_GOALS_MD: &str = include_str!("../assets/defaults/GOALS.md");

/// Result of a seeding operation.
#[derive(Debug)]
pub struct SeedResult {
    pub created: u32,
    pub updated: u32,
    pub skipped: u32,
    pub errors: u32,
}

fn load_default_asset(filename: &str) -> Option<&'static str> {
    match filename {
        "SOUL.md" => Some(DEFAULT_SOUL_MD),
        "USER_PROFILE.md" => Some(DEFAULT_USER_PROFILE_MD),
        "GOALS.md" => Some(DEFAULT_GOALS_MD),
        _ => {
            warn!(filename = filename, "db_seeding.unknown_asset");
            None
        }
    }
}

/// Seed default souls into the database.
pub async fn seed_default_souls(
    soul_repo: &dyn SoulRepository,
) -> Result<SeedResult, AppError> {
    let mut result = SeedResult {
        created: 0,
        updated: 0,
        skipped: 0,
        errors: 0,
    };

    for &(filename, name) in DEFAULT_SOULS {
        let content = match load_default_asset(filename) {
            Some(c) => c,
            None => {
                warn!(filename = filename, "db_seeding.asset_not_found");
                result.errors += 1;
                continue;
            }
        };

        match soul_repo.get_by_name(name).await {
            Ok(None) => {
                let soul = Soul::new(name.to_owned(), content.to_owned(), true);
                if let Err(e) = soul_repo.save(&soul).await {
                    warn!(name = name, error = %e, "db_seeding.soul_create_failed");
                    result.errors += 1;
                } else {
                    info!(name = name, "db_seeding.soul_created");
                    result.created += 1;
                }
            }
            Ok(Some(existing)) if existing.is_default => {
                if let Err(e) = soul_repo
                    .update(existing.id, Some(content), None, None, None)
                    .await
                {
                    warn!(name = name, error = %e, "db_seeding.soul_update_failed");
                    result.errors += 1;
                } else {
                    debug!(name = name, "db_seeding.soul_updated");
                    result.updated += 1;
                }
            }
            Ok(Some(_)) => {
                debug!(name = name, "db_seeding.soul_skipped_user_modified");
                result.skipped += 1;
            }
            Err(e) => {
                warn!(name = name, error = %e, "db_seeding.soul_check_failed");
                result.errors += 1;
            }
        }
    }

    info!(
        created = result.created,
        updated = result.updated,
        skipped = result.skipped,
        errors = result.errors,
        "db_seeding.souls_complete"
    );
    Ok(result)
}

/// Seed default user profiles into the database.
pub async fn seed_default_user_profiles(
    profile_repo: &dyn UserProfileRepository,
) -> Result<SeedResult, AppError> {
    let mut result = SeedResult {
        created: 0,
        updated: 0,
        skipped: 0,
        errors: 0,
    };

    for &(filename, name) in DEFAULT_USER_PROFILES {
        let content = match load_default_asset(filename) {
            Some(c) => c,
            None => {
                warn!(filename = filename, "db_seeding.user_profile_asset_not_found");
                result.errors += 1;
                continue;
            }
        };

        match profile_repo.get_by_name(name).await {
            Ok(None) => {
                let profile = UserProfile::new(name.to_owned(), content.to_owned(), true);
                if let Err(e) = profile_repo.save(&profile).await {
                    warn!(name = name, error = %e, "db_seeding.profile_create_failed");
                    result.errors += 1;
                } else {
                    info!(name = name, "db_seeding.user_profile_created");
                    result.created += 1;
                }
            }
            Ok(Some(existing)) if existing.is_default => {
                if let Err(e) = profile_repo
                    .update(existing.id, Some(content), None)
                    .await
                {
                    warn!(name = name, error = %e, "db_seeding.profile_update_failed");
                    result.errors += 1;
                } else {
                    debug!(name = name, "db_seeding.user_profile_updated");
                    result.updated += 1;
                }
            }
            Ok(Some(_)) => {
                debug!(name = name, "db_seeding.user_profile_skipped_user_modified");
                result.skipped += 1;
            }
            Err(e) => {
                warn!(name = name, error = %e, "db_seeding.profile_check_failed");
                result.errors += 1;
            }
        }
    }

    info!(
        created = result.created,
        updated = result.updated,
        skipped = result.skipped,
        errors = result.errors,
        "db_seeding.user_profiles_complete"
    );
    Ok(result)
}
