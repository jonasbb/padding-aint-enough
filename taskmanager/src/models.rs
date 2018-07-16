use chrono::{Local, NaiveDateTime};
use schema::tasks;

#[derive(Identifiable, Insertable, Queryable, AsChangeset, Debug, PartialEq, Eq)]
#[table_name = "tasks"]
pub struct Task {
    id: i32,
    priority: i32,
    name: String,
    domain: String,
    domain_counter: i32,
    pub(crate) state: TaskState,
    restart_count: i32,
    last_modified: NaiveDateTime,
    pub(crate) associated_data: Option<String>,
}

impl Task {
    #[inline]
    pub fn id(&self) -> i32 {
        self.id
    }

    #[inline]
    pub fn domain(&self) -> &str {
        &*self.domain
    }

    #[inline]
    pub fn domain_counter(&self) -> i32 {
        self.domain_counter
    }

    #[inline]
    pub fn state(&self) -> TaskState {
        self.state
    }

    #[inline]
    pub fn restart_count(&self) -> i32 {
        self.restart_count
    }

    pub(crate) fn advance(&mut self) {
        self.state.advance();
        self.last_modified = Local::now().naive_local();
    }

    pub(crate) fn restart(&mut self) {
        self.state.restart();
        self.last_modified = Local::now().naive_local();
        self.restart_count += 1;
    }

    pub(crate) fn abort(&mut self) {
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
    /// No associated data
    Created,
    /// [`Executor`] as associated data
    SubmittedToVm,
    ResultsCollectable,
    CheckQualitySingle,
    CheckQualityDomain,
    /// No associtated data
    Done,
    /// No associated data
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
