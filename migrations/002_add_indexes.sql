-- Performance indexes for commonly queried columns

-- Observations: frequently queried by created_at in learning loop and find_since
CREATE INDEX IF NOT EXISTS ix_observations_created_at ON observations(created_at);

-- Conversations: queried by (state, closed_at) in find_closed_since and get_last_closed
CREATE INDEX IF NOT EXISTS ix_conversations_state_closed_at ON conversations(state, closed_at);

-- Conversations: queried by (state, updated_at) in get_pending_or_active
CREATE INDEX IF NOT EXISTS ix_conversations_state_updated_at ON conversations(state, updated_at);

-- Agent jobs: queried for unreported terminal jobs by (reported, completed_at)
CREATE INDEX IF NOT EXISTS ix_agent_jobs_reported_completed ON agent_jobs(reported, completed_at);
