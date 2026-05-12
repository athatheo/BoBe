pub(crate) mod capture;
pub(crate) mod conversation;
pub(crate) mod engine;
pub(crate) mod events;
pub(crate) mod goals;
pub(crate) mod health;
pub(crate) mod local_runtime;
pub(crate) mod memories;
pub(crate) mod metrics;
pub(crate) mod settings;
pub(crate) mod souls;
pub(crate) mod tools_mcp;
pub(crate) mod user_profile;
pub(crate) mod voice;
pub(crate) mod voice_install;

pub(super) const fn default_true() -> bool {
    true
}
