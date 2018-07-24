CREATE UNIQUE INDEX IF NOT EXISTS tasks_pkey
    ON tasks (id);
CREATE INDEX IF NOT EXISTS tasks_state
    ON tasks (state, priority ASC)
    WHERE aborted = 0;
