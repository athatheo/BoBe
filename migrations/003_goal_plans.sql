-- Goal execution plans (Claude Agent SDK generated)
CREATE TABLE IF NOT EXISTS goal_plans (
    id          BLOB(16)    PRIMARY KEY NOT NULL,
    goal_id     BLOB(16)    NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    summary     TEXT        NOT NULL DEFAULT '',
    status      TEXT        NOT NULL DEFAULT 'pending_approval',
    failure_count INTEGER   NOT NULL DEFAULT 0,
    last_error  TEXT,
    created_at  DATETIME    NOT NULL DEFAULT (datetime('now')),
    updated_at  DATETIME    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_goal_plans_goal_status ON goal_plans(goal_id, status);
CREATE INDEX IF NOT EXISTS idx_goal_plans_status ON goal_plans(status);

-- Individual steps within a goal plan
CREATE TABLE IF NOT EXISTS goal_plan_steps (
    id          BLOB(16)    PRIMARY KEY NOT NULL,
    plan_id     BLOB(16)    NOT NULL REFERENCES goal_plans(id) ON DELETE CASCADE,
    step_order  INTEGER     NOT NULL DEFAULT 0,
    content     TEXT        NOT NULL DEFAULT '',
    status      TEXT        NOT NULL DEFAULT 'pending',
    result      TEXT,
    error       TEXT,
    started_at  DATETIME,
    completed_at DATETIME,
    created_at  DATETIME    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_goal_plan_steps_plan ON goal_plan_steps(plan_id, step_order);
