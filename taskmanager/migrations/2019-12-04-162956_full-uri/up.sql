TRUNCATE TABLE infos, tasks;

ALTER TABLE tasks RENAME COLUMN "domain" TO "website";

ALTER TABLE tasks RENAME COLUMN "domain_counter" TO "website_counter";

ALTER TABLE tasks
    ADD COLUMN "uri" text NOT NULL;

