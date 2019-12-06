-- This file should undo anything in `up.sql`
ALTER TABLE tasks RENAME COLUMN "website" TO "domain";

ALTER TABLE tasks RENAME COLUMN "website_counter" TO "domain_counter";

ALTER TABLE tasks
    DROP COLUMN "uri";

