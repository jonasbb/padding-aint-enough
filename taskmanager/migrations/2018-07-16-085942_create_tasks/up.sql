-- Your SQL goes here
CREATE TABLE tasks (
    id INTEGER NOT NULL PRIMARY KEY,
    priority INTEGER NOT NULL,
    name TEXT NOT NULL,
    domain TEXT NOT NULL,
    domain_counter INTEGER NOT NULL,
    state TEXT NOT NULL,
    restart_count INTEGER NOT NULL,
    last_modified DATETIME NOT NULL,
    associated_data TEXT
);
