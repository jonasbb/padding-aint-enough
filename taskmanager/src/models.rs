#![allow(proc_macro_derive_resolution_fallback)]

use crate::{
    schema::{infos, tasks},
    AddWebsiteConfig,
};
use chrono::{DateTime, Utc};
use diesel_derive_enum::DbEnum;

#[derive(Identifiable, Queryable, AsChangeset, QueryableByName, Debug, PartialEq, Eq)]
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
    website: String,
    website_counter: i32,
    pub(crate) state: TaskState,
    restart_count: i32,
    last_modified: DateTime<Utc>,
    pub(crate) associated_data: Option<String>,
    groupid: i32,
    groupsize: i32,
    uri: String,
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
    pub fn website(&self) -> &str {
        &self.website
    }

    #[inline]
    pub fn website_counter(&self) -> i32 {
        self.website_counter
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
        self.last_modified = Utc::now();
    }

    pub(crate) fn restart(&mut self) {
        self.state.restart();
        self.last_modified = Utc::now();
        self.restart_count += 1;
    }

    pub(crate) fn abort<'a>(&mut self, reason: &'a str) -> TaskAbort<'a> {
        self.state.abort();
        self.last_modified = Utc::now();
        TaskAbort {
            id: self.id,
            aborted: true,
            last_modified: self.last_modified,
            associated_data: reason,
        }
    }

    #[inline]
    pub fn groupid(&self) -> i32 {
        self.groupid
    }

    #[inline]
    pub fn groupsize(&self) -> i32 {
        self.groupsize
    }

    #[inline]
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

#[derive(Identifiable, AsChangeset, Debug, PartialEq, Eq)]
#[table_name = "tasks"]
pub struct TaskAbort<'a> {
    id: i32,
    aborted: bool,
    last_modified: DateTime<Utc>,
    associated_data: &'a str,
}

#[derive(Insertable, Debug, PartialEq, Eq)]
#[table_name = "tasks"]
pub struct TaskInsert<'a> {
    pub priority: i32,
    pub name: &'a str,
    pub website: &'a str,
    pub website_counter: i32,
    pub state: TaskState,
    pub restart_count: i32,
    pub last_modified: DateTime<Utc>,
    pub associated_data: Option<&'a str>,
    pub groupid: i32,
    pub groupsize: i32,
    pub uri: &'a str,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, DbEnum)]
#[PgType = "Task_State"]
#[DieselType = "Task_state"]
pub enum TaskState {
    /// No associated data
    Created,
    /// No associated data
    SubmittedToVm,
    /// No associated data
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
            SubmittedToVm => CheckQualitySingle,
            CheckQualitySingle => CheckQualityDomain,
            CheckQualityDomain => Done,
            Done => Done,
            Aborted => Aborted,
            ResultsCollectable => panic!("This state is outdated after the switch to Docker"),
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
    pub time: DateTime<Utc>,
    pub message: &'a str,
}

#[table_name = "tasks"]
#[derive(Clone, Debug, QueryableByName)]
pub struct WebsiteCounters {
    pub website: String,
    pub website_counter: i32,
    pub groupid: i32,
}

impl WebsiteCounters {
    pub fn into_add_website_config(self, groupsize: u8, uri: String) -> AddWebsiteConfig {
        AddWebsiteConfig {
            website: self.website,
            website_counter: self.website_counter,
            groupid: self.groupid,
            groupsize,
            uri,
        }
    }
}
