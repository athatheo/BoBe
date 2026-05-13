-- BoBe database schema.
--
-- Durable narrative state lives outside SQL: memory.md is the long-term
-- store, ~/.bobe/goals/<id>.md is one file per goal, captures get
-- vision-described and appended to memory.md. What stays in SQL is
-- runtime/operational state — conversations + turns (chat session log),
-- souls + user profiles (small structured docs the UI manages by name),
-- and cooldown tracking.

-- Drop dead tables left over from earlier installs.
DROP TABLE IF EXISTS goal_plan_steps;
DROP TABLE IF EXISTS goal_plans;
DROP TABLE IF EXISTS goals;
DROP TABLE IF EXISTS memories;
DROP TABLE IF EXISTS observations;
DROP TABLE IF EXISTS learning_state;
DROP TABLE IF EXISTS agent_jobs;

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

-- MCP server configurations are file-based (mcp.json), not stored in DB.
