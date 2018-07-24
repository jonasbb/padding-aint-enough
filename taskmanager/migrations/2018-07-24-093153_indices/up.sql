CREATE INDEX IF NOT EXISTS tasks_state
    ON tasks (state, priority ASC)
    WHERE aborted = false;
