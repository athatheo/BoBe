-- Initial database schema for BoBe
-- Mirrors the Python SQLAlchemy models

CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    closed_at TEXT,
    summary TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_conversations_state ON conversations(state);

CREATE TABLE IF NOT EXISTS conversation_turns (
    id TEXT PRIMARY KEY NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_conversation_turns_conversation_id ON conversation_turns(conversation_id);

CREATE TABLE IF NOT EXISTS observations (
    id TEXT PRIMARY KEY NOT NULL,
    source TEXT NOT NULL,
    content TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'general',
    embedding TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS memories (
    id TEXT PRIMARY KEY NOT NULL,
    content TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    category TEXT NOT NULL DEFAULT 'general',
    source TEXT NOT NULL,
    embedding TEXT,
    source_observation_id TEXT REFERENCES observations(id) ON DELETE SET NULL,
    source_conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_memories_memory_type ON memories(memory_type);
CREATE INDEX IF NOT EXISTS ix_memories_memory_type_created ON memories(memory_type, created_at);
CREATE INDEX IF NOT EXISTS ix_memories_enabled ON memories(enabled);

CREATE TABLE IF NOT EXISTS goals (
    id TEXT PRIMARY KEY NOT NULL,
    content TEXT NOT NULL,
    priority TEXT NOT NULL DEFAULT 'medium',
    source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    enabled INTEGER NOT NULL DEFAULT 1,
    inference_reason TEXT,
    embedding TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_goals_status_priority ON goals(status, priority);
CREATE INDEX IF NOT EXISTS ix_goals_source ON goals(source);
CREATE INDEX IF NOT EXISTS ix_goals_enabled ON goals(enabled);

CREATE TABLE IF NOT EXISTS souls (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_souls_name ON souls(name);

CREATE TABLE IF NOT EXISTS user_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_user_profiles_name ON user_profiles(name);

CREATE TABLE IF NOT EXISTS cooldown_state (
    id TEXT PRIMARY KEY NOT NULL,
    last_engagement TEXT,
    last_user_response TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS learning_state (
    id TEXT PRIMARY KEY NOT NULL,
    last_conversation_processed_at TEXT,
    last_context_processed_at TEXT,
    last_consolidation_at TEXT,
    last_pruning_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS mcp_server_configs (
    id TEXT PRIMARY KEY NOT NULL,
    server_name TEXT NOT NULL UNIQUE,
    command TEXT NOT NULL,
    args TEXT NOT NULL DEFAULT '[]',
    env TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    timeout_seconds REAL NOT NULL DEFAULT 30.0,
    is_default INTEGER NOT NULL DEFAULT 0,
    last_connected_at TEXT,
    last_error TEXT,
    excluded_tools TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_mcp_server_configs_name ON mcp_server_configs(server_name);

CREATE TABLE IF NOT EXISTS agent_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    profile_name TEXT NOT NULL,
    command TEXT NOT NULL,
    user_intent TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    working_directory TEXT NOT NULL,
    conversation_id TEXT,
    pid INTEGER,
    exit_code INTEGER,
    result_summary TEXT,
    raw_output_path TEXT,
    error_message TEXT,
    started_at TEXT,
    completed_at TEXT,
    cost_usd REAL,
    files_changed_json TEXT,
    agent_session_id TEXT,
    continuation_count INTEGER NOT NULL DEFAULT 0,
    reported INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_agent_jobs_status ON agent_jobs(status);
CREATE INDEX IF NOT EXISTS ix_agent_jobs_profile ON agent_jobs(profile_name);
CREATE INDEX IF NOT EXISTS ix_agent_jobs_reported ON agent_jobs(reported);
