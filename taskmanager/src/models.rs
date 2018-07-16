use chrono::{Local, NaiveDateTime};
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

impl Task {
    pub fn advance(&mut self) {
        self.state.advance();
        self.last_modified = Local::now().naive_local();
    }

    pub fn restart(&mut self) {
        self.state.restart();
        self.last_modified = Local::now().naive_local();
        self.restart_count += 1;
    }

    pub fn abort(&mut self) {
        self.state.abort();
        self.last_modified = Local::now().naive_local();
    }
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
    Aborted,
}

impl TaskState {
    fn advance(&mut self) {
        use self::TaskState::*;

        *self = match *self {
            Created => SubmittedToVm,
            SubmittedToVm => ResultsCollectable,
            ResultsCollectable => CheckQualitySingle,
            CheckQualitySingle => CheckQualityDomain,
            CheckQualityDomain => Done,
            Done => Done,
            Aborted => Aborted,
        }
    }

    fn restart(&mut self) {
        *self = TaskState::Created;
    }

    fn abort(&mut self) {
        *self = TaskState::Aborted;
    }
}
