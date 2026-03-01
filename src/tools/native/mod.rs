pub mod adapter;
pub mod base;

// Memory tools
pub mod create_memory;
pub mod get_goals;
pub mod get_recent_context;
pub mod get_souls;
pub mod search_context;
pub mod search_goal;
pub mod search_memories;
pub mod update_memory;

// Goal tools
pub mod archive_goal;
pub mod complete_goal;
pub mod create_goal;
pub mod pause_goal;
pub mod resume_goal;
pub mod update_goal;

// Plan tools
pub mod approve_plan;
pub mod reject_plan;

// File system tools
pub mod file_reader;
pub mod list_directory;
pub(crate) mod path_validation;
pub mod search_files;

// Research tools
pub mod browser_history;
pub mod fetch_url;

// System tools
pub mod cancel_coding_agent;
pub mod check_coding_agent;
pub mod discover_git_repos;
pub mod discover_installed_tools;
pub mod launch_coding_agent;
pub mod list_coding_agents;
