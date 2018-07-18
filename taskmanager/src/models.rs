#![allow(proc_macro_derive_resolution_fallback)]

use chrono::{Local, NaiveDateTime};
use schema::{infos, tasks};

#[allow(proc_macro_derive_resolution_fallback)]
#[derive(Identifiable, Queryable, AsChangeset, Debug, PartialEq, Eq)]
#[changeset_options(treat_none_as_null = "true")]
#[table_name = "tasks"]
/// A Queryable and Insertable Task for the database
///
/// WARNING!!! This struct must never have the `aborted` member, otherwise we could end up
/// overwriting the `aborted` flag in the database.
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
    pub fn name(&self) -> &str {
        &*self.name
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

    pub(crate) fn abort<'a>(&mut self, reason: &'a str) -> TaskAbort<'a> {
        self.state.abort();
        self.last_modified = Local::now().naive_local();
        TaskAbort {
            id: self.id,
            aborted: true,
            last_modified: self.last_modified,
            associated_data: reason,
        }
    }
}

#[derive(Identifiable, AsChangeset, Debug, PartialEq, Eq)]
#[table_name = "tasks"]
pub struct TaskAbort<'a> {
    id: i32,
    aborted: bool,
    last_modified: NaiveDateTime,
    associated_data: &'a str,
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
    /// [`ResultsCollectableData`] as associated data
    ResultsCollectable,
    /// No associated data
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

#[derive(Identifiable, Insertable, Associations, Debug, PartialEq, Eq)]
#[belongs_to(Task)]
#[table_name = "infos"]
pub struct InfoInsert<'a> {
    pub id: Option<i32>,
    pub task_id: i32,
    pub time: NaiveDateTime,
    pub message: &'a str,
}
