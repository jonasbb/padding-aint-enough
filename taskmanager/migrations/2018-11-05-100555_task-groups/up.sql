-- Create two columns, set some initial values for old rows, then make them NOT NULL
ALTER TABLE tasks
    ADD COLUMN groupid INTEGER;

ALTER TABLE tasks
    ADD COLUMN groupsize INTEGER;

UPDATE
    tasks
SET
    groupid = 0,
    groupsize = 10;

ALTER TABLE tasks ALTER COLUMN groupid SET NOT NULL;

ALTER TABLE tasks ALTER COLUMN groupsize SET NOT NULL;

CREATE INDEX IF NOT EXISTS tasks_groups ON tasks ("domain", groupid);

