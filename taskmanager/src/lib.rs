#![recursion_limit = "128"]

extern crate chrono;
#[macro_use]
extern crate diesel;
extern crate diesel_derive_enum;
#[macro_use]
extern crate diesel_migrations;
extern crate failure;
extern crate log;
extern crate misc_utils;
extern crate serde;
extern crate toml;

use chrono::{Duration, Utc};
use diesel::prelude::*;
use failure::{bail, Error, ResultExt};
use log::info;
use misc_utils::fs::file_open_read;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Display},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub mod models;
pub mod schema;

// This createa a module called `embedded_migrations` which can then be used to run them.
embed_migrations!("./migrations");

/// Maximal number of restarts which are happening for a task.
///
/// The full number of tries which are executed are `MAX_RESTART_COUNT` + 1, for the initial try.
const MAX_RESTART_COUNT: i32 = 3;

type TasksColumnType = (
    schema::tasks::id,
    schema::tasks::priority,
    schema::tasks::name,
    schema::tasks::domain,
    schema::tasks::domain_counter,
    schema::tasks::state,
    schema::tasks::restart_count,
    schema::tasks::last_modified,
    schema::tasks::associated_data,
    schema::tasks::groupid,
    schema::tasks::groupsize,
);
const TASKS_COLUMNS: TasksColumnType = (
    schema::tasks::id,
    schema::tasks::priority,
    schema::tasks::name,
    schema::tasks::domain,
    schema::tasks::domain_counter,
    schema::tasks::state,
    schema::tasks::restart_count,
    schema::tasks::last_modified,
    schema::tasks::associated_data,
    schema::tasks::groupid,
    schema::tasks::groupsize,
);

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AddDomainConfig {
    pub(crate) domain: String,
    pub(crate) domain_counter: i32,
    pub(crate) groupid: i32,
    pub(crate) groupsize: u8,
}

impl AddDomainConfig {
    pub fn new(
        domain: impl Into<String>,
        domain_counter: i32,
        groupid: i32,
        groupsize: u8,
    ) -> Self {
        assert!(domain_counter >= 0);
        assert!(groupid >= 0);
        Self {
            domain: domain.into(),
            domain_counter,
            groupid,
            groupsize,
        }
    }
}

#[derive(Clone)]
pub struct TaskManager {
    db_connection: Arc<Mutex<PgConnection>>,
}

impl Debug for TaskManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TaskManager")
            .field("db_connection", &"<PgConnection>")
            .finish()
    }
}

impl TaskManager {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(database: &str) -> Result<Self, Error> {
        let conn = PgConnection::establish(database)?;
        conn.execute("SET lock_timeout TO 30000")?;
        conn.execute("SET statement_timeout TO 90000")?;
        let db_connection = Arc::new(Mutex::new(conn));
        Ok(Self { db_connection })
    }

    pub fn run_migrations(&self) -> Result<(), Error> {
        let conn = self.db_connection.lock().unwrap();
        info!("Run database migrations");
        embedded_migrations::run_with_output(&*conn, &mut std::io::stdout())?;
        Ok(())
    }

    pub fn delete_all(&self) -> Result<(), Error> {
        let conn = self.db_connection.lock().unwrap();
        conn.transaction::<(), _, _>(|| {
            conn.execute("TRUNCATE TABLE infos, tasks;")
                .context("Trying to delete tables `infos` and `tasks`")?;
            Ok(())
        })
    }

    fn update_tasks<'a, T>(&self, conn: &PgConnection, tasks: T) -> Result<(), Error>
    where
        T: IntoIterator<Item = &'a models::Task>,
    {
        conn.transaction::<(), Error, _>(|| {
            for task in tasks {
                diesel::update(task)
                    .set(task)
                    .execute(conn)
                    .context("Cannot update task")?;
            }
            Ok(())
        })
    }

    pub fn add_domains<I>(&self, domains: I, initial_priority: i32) -> Result<(), Error>
    where
        I: IntoIterator<Item = AddDomainConfig>,
    {
        let conn = self.db_connection.lock().unwrap();
        conn.transaction(|| {
            let mut prio = 0;
            for config in domains {
                for i in 0..config.groupsize {
                    let dc = config.domain_counter + i32::from(i);
                    let row = models::TaskInsert {
                        priority: prio + i32::from(i) + initial_priority,
                        name: &format!("{}-{}-{}", config.domain, dc, config.groupid),
                        domain: &config.domain,
                        domain_counter: dc,
                        state: models::TaskState::Created,
                        restart_count: 0,
                        last_modified: Utc::now(),
                        associated_data: None,
                        groupid: config.groupid,
                        groupsize: i32::from(config.groupsize),
                    };
                    diesel::insert_into(schema::tasks::table)
                        .values(&row)
                        .execute(&*conn)
                        .context("Error creating new task")?;
                }

                prio += i32::from(config.groupsize);
            }
            Ok(())
        })
    }

    /// Return a task which waits for a VM to be executed
    pub fn get_task_for_vm(&self) -> Result<Option<models::Task>, Error> {
        use crate::schema::tasks::dsl::{aborted, priority, state, tasks};

        let conn = self.db_connection.lock().unwrap();
        conn.transaction(|| {
            let res = tasks
                .filter(state.eq(models::TaskState::Created))
                .filter(aborted.eq(false))
                .order_by(priority.asc())
                .limit(1)
                .select(TASKS_COLUMNS)
                .load::<models::Task>(&*conn)
                .context("Cannot retrieve task from database")?;

            // we only fetch one task, so this next is sufficient to retrieve all data
            let mut task = res.into_iter().next();
            if let Some(ref mut task) = &mut task {
                task.advance();
                diesel::update(&*task)
                    .set(&*task)
                    .execute(&*conn)
                    .context("Cannot update task")?;
            }
            Ok(task)
        })
    }

    /// Return all tasks which did not make any progress for a too long time
    pub fn get_stale_tasks(&self) -> Result<Vec<models::Task>, Error> {
        use crate::schema::tasks::dsl::{aborted, last_modified, state, tasks};

        let conn = self.db_connection.lock().unwrap();
        conn.transaction(|| {
            let res = tasks
                .filter(state.ne(models::TaskState::Created))
                .filter(state.ne(models::TaskState::Done))
                .filter(state.ne(models::TaskState::Aborted))
                .filter(aborted.eq(false))
                .filter(last_modified.lt(Utc::now() - Duration::hours(2)))
                .select(TASKS_COLUMNS)
                .load::<models::Task>(&*conn)
                .context("Cannot retrieve task from database")?;
            Ok(res)
        })
    }

    pub fn finished_task_for_vm(&self, task: &mut models::Task) -> Result<(), Error> {
        if task.state != models::TaskState::SubmittedToVm {
            bail!("To complete a VM task it must be in the SubmittedToVm state.")
        }

        // Update task state
        task.advance();
        task.associated_data = None;

        let conn = self.db_connection.lock().unwrap();
        conn.transaction(|| self.update_tasks(&*conn, Some(&*task)))
    }

    pub fn results_need_sanity_check_single(&self) -> Result<Vec<models::Task>, Error> {
        use crate::schema::tasks::dsl::{aborted, priority, state, tasks};

        let conn = self.db_connection.lock().unwrap();
        conn.transaction(|| {
            Ok(tasks
                .filter(state.eq(models::TaskState::CheckQualitySingle))
                .filter(aborted.eq(false))
                .order_by(priority.asc())
                .select(TASKS_COLUMNS)
                .load::<models::Task>(&*conn)
                .context("Cannot retrieve tasks from database")?)
        })
    }

    pub fn mark_results_checked_single(&self, task: &mut models::Task) -> Result<(), Error> {
        if task.state != models::TaskState::CheckQualitySingle {
            bail!("To check results they must be in the CheckQualitySingle state.")
        }

        // Update task state
        task.advance();
        task.associated_data = None;

        let conn = self.db_connection.lock().unwrap();
        conn.transaction(|| self.update_tasks(&conn, Some(&*task)))
    }

    pub fn results_need_sanity_check_domain(&self) -> Result<Option<Vec<models::Task>>, Error> {
        use diesel::dsl::sql_query;

        let conn = self.db_connection.lock().unwrap();
        let tasks = conn.transaction::<Vec<models::Task>, Error, _>(|| {
            Ok(sql_query(
                r#"SELECT
                t.id,
                t.priority,
                t.name,
                t.domain,
                t.domain_counter,
                t.state,
                t.restart_count,
                t.last_modified,
                t.associated_data,
                t.groupid,
                t.groupsize
            FROM (
                SELECT domain, groupid
                FROM tasks
                WHERE state = 'check_quality_domain'
                    AND aborted = false
                GROUP BY domain, groupid
                HAVING count(*) = MAX(groupsize)
                LIMIT 1
            ) AS s
            JOIN tasks t
                ON s.domain = t.domain
               AND s.groupid = t.groupid

            ORDER BY
                t.domain,
                priority ASC
            ;"#,
            )
            .load::<models::Task>(&*conn)
            .context("Cannot retrieve tasks from database")?)
        })?;

        if tasks.is_empty() {
            Ok(None)
        } else {
            assert_eq!(
                tasks.len(),
                tasks[0].groupsize() as usize,
                "The number of tasks MUST match the groupsize."
            );
            Ok(Some(tasks))
        }
    }

    pub fn mark_results_checked_domain(&self, tasks: &mut Vec<models::Task>) -> Result<(), Error> {
        for task in &*tasks {
            if task.state != models::TaskState::CheckQualityDomain {
                bail!("To check results they must be in the CheckQualityDomain state.")
            }
        }

        for mut task in &mut *tasks {
            // Update task state
            task.advance();
            task.associated_data = None;
        }

        let msg = format!(
            "Finished domain {} groupid {}",
            tasks[0].domain(),
            tasks[0].groupid()
        );
        let row = models::InfoInsert {
            id: None,
            task_id: tasks[0].id(),
            time: Utc::now(),
            message: &*msg,
        };
        let conn = self.db_connection.lock().unwrap();
        conn.transaction(|| {
            diesel::insert_into(schema::infos::table)
                .values(&row)
                .execute(&*conn)
                .context("Error creating new info")?;
            self.update_tasks(&conn, &*tasks)
        })
    }

    pub fn restart_task(&self, task: &mut models::Task, reason: &Display) -> Result<(), Error> {
        task.restart();
        task.associated_data = None;

        let conn = self.db_connection.lock().unwrap();
        if task.restart_count() <= MAX_RESTART_COUNT {
            // The task is still allowed to be restarted
            let msg = format!("Restart task {} because {}", task.name(), reason);
            conn.transaction(|| {
                self.update_tasks(&conn, Some(&*task))?;

                let row = models::InfoInsert {
                    id: None,
                    task_id: task.id(),
                    time: Utc::now(),
                    message: &*msg,
                };
                diesel::insert_into(schema::infos::table)
                    .values(&row)
                    .execute(&*conn)
                    .context("Error creating new info")?;
                Ok(())
            })
        } else {
            use crate::schema::tasks::dsl::{domain, groupid, tasks};

            // We must abort all tasks for this domain
            let msg = format!("Too many restarts for task {}, abort domain.", task.name());

            conn.transaction(|| {
                // get all tasks for the same domain
                let other_tasks = tasks
                    .filter(domain.eq(task.domain()))
                    .filter(groupid.eq(task.groupid()))
                    .select(TASKS_COLUMNS)
                    .load::<models::Task>(&*conn)
                    .context("Cannot retrieve task from database")?;

                for mut other_task in other_tasks {
                    let abort_task = other_task.abort(&msg);
                    diesel::update(&abort_task)
                        .set(&abort_task)
                        .execute(&*conn)
                        .context("Cannot update task")?;
                    let row = models::InfoInsert {
                        id: None,
                        task_id: other_task.id(),
                        time: Utc::now(),
                        message: &*msg,
                    };
                    diesel::insert_into(schema::infos::table)
                        .values(&row)
                        .execute(&*conn)
                        .context("Error creating new task")?;
                }
                Ok(())
            })
        }
    }

    pub fn restart_tasks(&self, tasks: &mut [models::Task], reason: &Display) -> Result<(), Error> {
        // check that all tasks belong to the same domain
        for task in &*tasks {
            assert_eq!(
                tasks[0].domain(),
                task.domain(),
                "restart_tasks only works if all tasks belong to the same domain"
            );
            assert_eq!(
                tasks[0].groupid(),
                task.groupid(),
                "restart_tasks only works if all tasks belong to the same groupid"
            );
        }

        let mut abort_tasks = false;
        for task in &mut *tasks {
            task.restart();
            task.associated_data = None;

            if task.restart_count() > MAX_RESTART_COUNT {
                abort_tasks = true;
            }
        }

        let conn = self.db_connection.lock().unwrap();
        if !abort_tasks {
            // The task is still allowed to be restarted
            conn.transaction(|| {
                self.update_tasks(&conn, tasks.iter().map(|t| &*t))?;

                for task in tasks {
                    let msg = format!("Restart task {} because {}", task.name(), reason);
                    let row = models::InfoInsert {
                        id: None,
                        task_id: task.id(),
                        time: Utc::now(),
                        message: &*msg,
                    };
                    diesel::insert_into(schema::infos::table)
                        .values(&row)
                        .execute(&*conn)
                        .context("Error creating new task")?;
                }
                Ok(())
            })
        } else {
            // We must abort all tasks for this domain
            let msg = format!(
                "Too many restarts for domain {} groupid {}",
                tasks[0].domain(),
                tasks[0].groupid()
            );

            conn.transaction(|| {
                for task in tasks {
                    let abort_task = task.abort(&msg);
                    diesel::update(&abort_task)
                        .set(&abort_task)
                        .execute(&*conn)
                        .context("Cannot update task")?;
                    let row = models::InfoInsert {
                        id: None,
                        task_id: task.id(),
                        time: Utc::now(),
                        message: &*msg,
                    };
                    diesel::insert_into(schema::infos::table)
                        .values(&row)
                        .execute(&*conn)
                        .context("Error creating new task")?;
                }
                Ok(())
            })
        }
    }

    pub fn get_domain_state(
        &self,
        domains: &[impl AsRef<str>],
    ) -> Result<Vec<models::DomainCounters>, Error> {
        use diesel::{dsl::sql_query, sql_types::*};

        let conn = self.db_connection.lock().unwrap();
        let domain_counters =
            conn.transaction::<Vec<Vec<models::DomainCounters>>, Error, _>(|| {
                domains
                    .iter()
                    .map(|domain| -> Result<Vec<models::DomainCounters>, Error> {
                        Ok(sql_query(
                            r#"SELECT
                            domain,
                            MAX(domain_counter) + 1 as domain_counter,
                            MAX(groupid) + 1 as groupid
                        FROM tasks
                        WHERE
                            domain = $1
                        GROUP BY
                            domain
                        ;"#,
                        )
                        .bind::<Text, _>(domain.as_ref())
                        .load::<models::DomainCounters>(&*conn)
                        .with_context(|_| {
                            format!(
                                "Cannot retrieve domain counters from database for domain '{}'",
                                domain.as_ref(),
                            )
                        })?)
                    })
                    .collect()
            })?;

        Ok(domain_counters.into_iter().flat_map(|x| x).collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub working_directory: PathBuf,
    pub database: PathBuf,
    pub per_domain_datasets: u8,
    pub per_domain_datasets_repeated_measurements: u8,
    pub max_allowed_dist_difference: f32,
    pub max_allowed_dist_difference_abs: usize,
    pub initial_priority: i32,
    pub num_executors: u8,
    pub refresh_cache_seconds: u32,
    pub docker_image: String,
}

impl Config {
    pub fn try_load_config(path: &Path) -> Result<Config, Error> {
        let mut content = String::new();
        file_open_read(path)?
            .read_to_string(&mut content)
            .context("Cannot read config file")?;
        Ok(toml::from_str(&content)?)
    }

    pub fn get_database_path(&self) -> PathBuf {
        self.database.clone()
    }

    pub fn get_collected_results_path(&self) -> PathBuf {
        self.working_directory.join("unprocessed")
    }

    pub fn get_results_path(&self) -> PathBuf {
        self.working_directory.join("processed")
    }

    pub fn get_cache_file(&self) -> PathBuf {
        self.working_directory.join("cache.dump")
    }

    pub fn get_prefetch_file(&self) -> PathBuf {
        self.working_directory.join("alexa-top30k-eff-tlds.txt")
    }
}
