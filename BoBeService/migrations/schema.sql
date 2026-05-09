-- BoBe database schema.
--
-- After the Copilot SDK pivot, durable narrative state moved out of
-- SQL: memory.md owns long-term context, ~/.bobe/goals/<id>.md is one
-- file per goal, screen captures get vision-described and appended
-- straight to memory.md (no observations row). What stays in SQL is
-- runtime/operational state — conversations and turns (the chat
-- session log), souls + user profiles (small structured docs the UI
-- still manages by name), cooldown tracking, and coding-agent job
-- bookkeeping.

-- Drop dead tables left over from earlier installs. Each is gone in
-- the SDK world; the embeddings, similarity search, and goal-worker
-- subsystems they backed have all been deleted.
DROP TABLE IF EXISTS goal_plan_steps;
DROP TABLE IF EXISTS goal_plans;
DROP TABLE IF EXISTS goals;
DROP TABLE IF EXISTS memories;
DROP TABLE IF EXISTS observations;
DROP TABLE IF EXISTS learning_state;

-- Conversations (chat session log — a daily/inactivity-rotated thread).
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

-- Soul documents (personality).
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

-- User profiles.
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

-- Cooldown tracking (when did BoBe last engage / hear from user).
CREATE TABLE IF NOT EXISTS cooldown_state (
    id BLOB PRIMARY KEY NOT NULL,
    last_engagement TEXT,
    last_user_response TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Agent jobs (coding-agent invocations driven by `agent_job_trigger`).
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

-- MCP server configurations are file-based (mcp.json), not stored in DB.
