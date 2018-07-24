CREATE TYPE Task_State AS ENUM (
    'created',
    'submitted_to_vm',
    'results_collectable',
    'check_quality_single',
    'check_quality_domain',
    'done',
    'aborted'
);

CREATE TABLE tasks (
    id SERIAL PRIMARY KEY,
    priority INTEGER NOT NULL,
    name TEXT NOT NULL,
    domain TEXT NOT NULL,
    domain_counter INTEGER NOT NULL,
    state Task_State NOT NULL,
    restart_count INTEGER NOT NULL,
    aborted BOOLEAN DEFAULT false NOT NULL,
    last_modified TIMESTAMP WITH TIME ZONE NOT NULL,
    associated_data TEXT
);

CREATE TABLE infos (
    id INTEGER NOT NULL PRIMARY KEY,
    task_id INTEGER NOT NULL,
    time TIMESTAMP WITH TIME ZONE NOT NULL,
    message TEXT NOT NULL,

    FOREIGN KEY (task_id) REFERENCES tasks(id)
);
