-- Your SQL goes here
CREATE TABLE tasks (
    id INTEGER NOT NULL PRIMARY KEY,
    priority INTEGER NOT NULL,
    name TEXT NOT NULL,
    domain TEXT NOT NULL,
    domain_counter INTEGER NOT NULL,
    state TEXT NOT NULL,
    restart_count INTEGER NOT NULL,
    aborted INTEGER DEFAULT 0 NOT NULL,
    last_modified DATETIME NOT NULL,
    associated_data TEXT
);

CREATE TABLE infos (
    id INTEGER NOT NULL PRIMARY KEY,
    task_id INTEGER NOT NULL,
    time DATETIME NOT NULL,
    message TEXT NOT NULL,

    FOREIGN KEY (task_id) REFERENCES tasks(id)
);
