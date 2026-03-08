-- BoBe database schema v1

-- Conversations
CREATE TABLE IF NOT EXISTS conversations (
    id BLOB PRIMARY KEY NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    closed_at TEXT,
    summary TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_conversations_state ON conversations(state);
CREATE INDEX IF NOT EXISTS ix_conversations_state_closed_at ON conversations(state, closed_at);
CREATE INDEX IF NOT EXISTS ix_conversations_state_updated_at ON conversations(state, updated_at);

CREATE TABLE IF NOT EXISTS conversation_turns (
    id BLOB PRIMARY KEY NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    conversation_id BLOB NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_conversation_turns_conversation_id ON conversation_turns(conversation_id);

-- Observations (screen captures, messages, context)
CREATE TABLE IF NOT EXISTS observations (
    id BLOB PRIMARY KEY NOT NULL,
    source TEXT NOT NULL,
    content TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'general',
    embedding TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_observations_created_at ON observations(created_at);

-- Memories (short-term and long-term)
CREATE TABLE IF NOT EXISTS memories (
    id BLOB PRIMARY KEY NOT NULL,
    content TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    category TEXT NOT NULL DEFAULT 'general',
    source TEXT NOT NULL,
    embedding TEXT,
    source_observation_id BLOB REFERENCES observations(id) ON DELETE SET NULL,
    source_conversation_id BLOB REFERENCES conversations(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_memories_memory_type ON memories(memory_type);
CREATE INDEX IF NOT EXISTS ix_memories_memory_type_created ON memories(memory_type, created_at);
CREATE INDEX IF NOT EXISTS ix_memories_enabled ON memories(enabled);

-- Goals
CREATE TABLE IF NOT EXISTS goals (
    id BLOB PRIMARY KEY NOT NULL,
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

-- Goal execution plans
CREATE TABLE IF NOT EXISTS goal_plans (
    id BLOB PRIMARY KEY NOT NULL,
    goal_id BLOB NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending_approval',
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_goal_plans_goal_status ON goal_plans(goal_id, status);
CREATE INDEX IF NOT EXISTS ix_goal_plans_status ON goal_plans(status);

CREATE TABLE IF NOT EXISTS goal_plan_steps (
    id BLOB PRIMARY KEY NOT NULL,
    plan_id BLOB NOT NULL REFERENCES goal_plans(id) ON DELETE CASCADE,
    step_order INTEGER NOT NULL DEFAULT 0,
    content TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    result TEXT,
    error TEXT,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_goal_plan_steps_plan ON goal_plan_steps(plan_id, step_order);

-- Data migration: fix legacy "inprogress" values (was lowercase, now snake_case)
UPDATE goal_plan_steps SET status = 'in_progress' WHERE status = 'inprogress';

-- Soul documents (personality)
CREATE TABLE IF NOT EXISTS souls (
    id BLOB PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_souls_name ON souls(name);

-- User profiles
CREATE TABLE IF NOT EXISTS user_profiles (
    id BLOB PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS ix_user_profiles_name ON user_profiles(name);

-- Runtime state
CREATE TABLE IF NOT EXISTS cooldown_state (
    id BLOB PRIMARY KEY NOT NULL,
    last_engagement TEXT,
    last_user_response TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS learning_state (
    id BLOB PRIMARY KEY NOT NULL,
    last_conversation_processed_at TEXT,
    last_context_processed_at TEXT,
    last_consolidation_at TEXT,
    last_pruning_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- MCP server configurations are file-based (mcp.json), not stored in DB.

-- Agent jobs (coding agent)
CREATE TABLE IF NOT EXISTS agent_jobs (
    id BLOB PRIMARY KEY NOT NULL,
    profile_name TEXT NOT NULL,
    command TEXT NOT NULL,
    user_intent TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    working_directory TEXT NOT NULL,
    conversation_id BLOB,
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
CREATE INDEX IF NOT EXISTS ix_agent_jobs_reported_completed ON agent_jobs(reported, completed_at);
