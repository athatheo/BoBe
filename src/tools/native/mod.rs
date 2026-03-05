pub(crate) mod adapter;
pub(crate) mod base;

pub(crate) mod create_memory;
pub(crate) mod get_goals;
pub(crate) mod get_recent_context;
pub(crate) mod get_souls;
pub(crate) mod search_context;
pub(crate) mod search_goal;
pub(crate) mod search_memories;
pub(crate) mod update_memory;

pub(crate) mod archive_goal;
pub(crate) mod complete_goal;
pub(crate) mod create_goal;
pub(crate) mod pause_goal;
pub(crate) mod resume_goal;
pub(crate) mod update_goal;

pub(crate) mod approve_plan;
pub(crate) mod reject_plan;

pub(crate) mod file_reader;
pub(crate) mod list_directory;
pub(crate) mod path_validation;
pub(crate) mod search_files;

pub(crate) mod browser_history;
pub(crate) mod fetch_url;

pub(crate) mod cancel_coding_agent;
pub(crate) mod check_coding_agent;
pub(crate) mod discover_git_repos;
pub(crate) mod discover_installed_tools;
pub(crate) mod launch_coding_agent;
pub(crate) mod list_coding_agents;
