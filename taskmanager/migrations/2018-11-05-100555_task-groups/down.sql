ALTER TABLE tasks DROP COLUMN groupid;

ALTER TABLE tasks DROP COLUMN groupsize;

DROP INDEX tasks_groups;

