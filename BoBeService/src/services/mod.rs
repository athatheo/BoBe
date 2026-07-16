pub(crate) mod goals;
pub(crate) mod mcp_config_service;
pub(crate) mod ollama;
pub(crate) mod souls_service;
pub(crate) mod storage_retention;
pub(crate) mod user_profile_service;

use crate::error::AppError;

/// Shared CRUD-delete outcome used by `*_service::delete` methods.
/// Lives at module-level so a new `*Service` doesn't reach into a
/// sibling (`souls::souls_service`) just for the enum.
pub(crate) enum DeleteOutcome {
    Deleted,
    NotFound,
}

/// Validation shared by Soul and UserProfile create paths. Both store
/// markdown content under a unique name with the same minimum-length
/// requirement, so factoring this out keeps the rule in one place.
pub(crate) fn validate_markdown_doc_input(
    name: &str,
    content: &str,
    min_content_len: usize,
) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    if content.len() < min_content_len {
        return Err(AppError::Validation(format!(
            "content must be at least {min_content_len} characters"
        )));
    }
    Ok(())
}
