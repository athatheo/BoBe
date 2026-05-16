pub(crate) mod goals;
pub(crate) mod mcp_config_service;
pub(crate) mod ollama_install_service;
pub(crate) mod souls_service;
pub(crate) mod user_profile_service;

/// Shared CRUD-delete outcome used by `*_service::delete` methods.
/// Lives at module-level so a new `*Service` doesn't reach into a
/// sibling (`souls::souls_service`) just for the enum.
pub(crate) enum DeleteOutcome {
    Deleted,
    NotFound,
}
