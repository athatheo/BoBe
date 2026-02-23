pub mod base;
pub mod adapter;

// Memory tools
pub mod search_memories;
pub mod search_context;
pub mod search_goal;
pub mod get_goals;
pub mod get_souls;
pub mod get_recent_context;
pub mod create_memory;
pub mod update_memory;

// Goal tools
pub mod create_goal;
pub mod update_goal;
pub mod complete_goal;
pub mod archive_goal;

// File system tools
pub mod file_reader;
pub mod list_directory;
pub mod search_files;

// Research tools
pub mod fetch_url;
pub mod browser_history;

// System tools
pub mod discover_git_repos;
pub mod discover_installed_tools;
pub mod launch_coding_agent;
pub mod check_coding_agent;
pub mod cancel_coding_agent;
pub mod list_coding_agents;

