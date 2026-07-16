CREATE TABLE goals (
 id BLOB PRIMARY KEY, content TEXT NOT NULL, priority TEXT NOT NULL, source TEXT NOT NULL,
 status TEXT NOT NULL, enabled INTEGER NOT NULL, inference_reason TEXT, embedding TEXT,
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE goal_plans (
 id BLOB PRIMARY KEY, goal_id BLOB NOT NULL, summary TEXT NOT NULL, status TEXT NOT NULL,
 failure_count INTEGER NOT NULL, last_error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE goal_plan_steps (
 id BLOB PRIMARY KEY, plan_id BLOB NOT NULL, step_order INTEGER NOT NULL, content TEXT NOT NULL,
 status TEXT NOT NULL, result TEXT, error TEXT, started_at TEXT, completed_at TEXT, created_at TEXT NOT NULL
);
CREATE TABLE memories (
 id BLOB PRIMARY KEY, content TEXT NOT NULL, memory_type TEXT NOT NULL, enabled INTEGER NOT NULL,
 category TEXT NOT NULL, source TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE observations (
 id BLOB PRIMARY KEY, source TEXT NOT NULL, content TEXT NOT NULL, category TEXT NOT NULL,
 metadata TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
INSERT INTO goals VALUES (X'00112233445566778899AABBCCDDEEFF', 'Ship the migration', 'high', 'user', 'active', 1, 'Data must survive', NULL, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
INSERT INTO goal_plans VALUES (X'11112222333344445555666677778888', X'00112233445566778899AABBCCDDEEFF', 'Safe rollout', 'active', 0, NULL, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
INSERT INTO goal_plan_steps VALUES (X'99992222333344445555666677778888', X'11112222333344445555666677778888', 0, 'Back up first', 'completed', 'done', NULL, NULL, '2026-01-02T00:00:00Z', '2026-01-01T00:00:00Z');
INSERT INTO memories VALUES (X'10112233445566778899AABBCCDDEEFF', 'remember this', 'long_term', 1, 'general', 'user', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
INSERT INTO observations VALUES (X'20112233445566778899AABBCCDDEEFF', 'capture', 'observed this', 'general', '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
