use chrono::NaiveDateTime;
use schema::tasks;

#[derive(Identifiable, Insertable, Queryable, Debug, PartialEq, Eq)]
#[table_name = "tasks"]
pub struct Task {
    pub id: i32,
    pub priority: i32,
    pub name: String,
    pub domain: String,
    pub domain_counter: i32,
    pub state: TaskState,
    pub restart_count: i32,
    pub last_modified: NaiveDateTime,
    pub associated_data: Option<String>,
}

#[derive(Insertable, Debug, PartialEq, Eq)]
#[table_name = "tasks"]
pub struct TaskInsert<'a> {
    pub priority: i32,
    pub name: &'a str,
    pub domain: &'a str,
    pub domain_counter: i32,
    pub state: TaskState,
    pub restart_count: i32,
    pub last_modified: NaiveDateTime,
    pub associated_data: Option<&'a str>,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, DbEnum)]
pub enum TaskState {
    Created,
    SubmittedToVm,
    ResultsCollectable,
    CheckQualitySingle,
    CheckQualityDomain,
    Done,
}
