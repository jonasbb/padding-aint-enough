#![recursion_limit = "128"]

#[macro_use]
extern crate diesel;

#[macro_use]
extern crate diesel_migrations;

use anyhow::{bail, Context as _, Error};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use log::info;
use misc_utils::fs::read_to_string;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
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
    schema::tasks::website,
    schema::tasks::website_counter,
    schema::tasks::state,
    schema::tasks::restart_count,
    schema::tasks::last_modified,
    schema::tasks::associated_data,
    schema::tasks::groupid,
    schema::tasks::groupsize,
    schema::tasks::uri,
);
const TASKS_COLUMNS: TasksColumnType = (
    schema::tasks::id,
    schema::tasks::priority,
    schema::tasks::name,
    schema::tasks::website,
    schema::tasks::website_counter,
    schema::tasks::state,
    schema::tasks::restart_count,
    schema::tasks::last_modified,
    schema::tasks::associated_data,
    schema::tasks::groupid,
    schema::tasks::groupsize,
    schema::tasks::uri,
);

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AddWebsiteConfig {
    pub(crate) website: String,
    pub(crate) website_counter: i32,
    pub(crate) groupid: i32,
    pub(crate) groupsize: u8,
    pub(crate) uri: String,
}

impl AddWebsiteConfig {
    pub fn new(
        website: impl Into<String>,
        website_counter: i32,
        groupid: i32,
        groupsize: u8,
        uri: impl Into<String>,
    ) -> Self {
        assert!(website_counter >= 0);
        assert!(groupid >= 0);
        Self {
            website: website.into(),
            website_counter,
            groupid,
            groupsize,
            uri: uri.into(),
        }
    }
}

#[derive(Clone)]
pub struct TaskManager {
    db_connection: Arc<Mutex<PgConnection>>,
}

impl Debug for TaskManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("TaskManager")
            .field("db_connection", &"<PgConnection>")
            .finish()
    }
}

impl TaskManager {
    pub fn new(database: &str) -> Result<Self, Error> {
        let conn = PgConnection::establish(database)?;
        conn.execute("SET lock_timeout TO 30000")?;
        conn.execute("SET statement_timeout TO 90000")?;
        let db_connection = Arc::new(Mutex::new(conn));
        Ok(Self { db_connection })
    }

    /// Perform database schema migration steps
    pub fn run_migrations(&self) -> Result<(), Error> {
        let conn = self.db_connection.lock().unwrap();
        info!("Run database migrations");
        embedded_migrations::run_with_output(&*conn, &mut std::io::stdout())?;
        Ok(())
    }

    /// Truncate all tables to create a fresh database state
    pub fn delete_all(&self) -> Result<(), Error> {
        let conn = self.db_connection.lock().unwrap();
        conn.transaction::<(), _, _>(|| {
            conn.execute("TRUNCATE TABLE infos, tasks;")
                .context("Trying to delete tables `infos` and `tasks`")?;
            Ok(())
        })
    }

    /// Update all the tasks which are passed through `tasks`
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

    /// Add a list of URIs to the database
    ///
    /// The tasks will get an increasing priority starting from `initial_priority`.
    pub fn add_uris<I>(&self, websites: I, initial_priority: i32) -> Result<(), Error>
    where
        I: IntoIterator<Item = AddWebsiteConfig>,
    {
        let conn = self.db_connection.lock().unwrap();
        conn.transaction(|| {
            let mut prio = 0;
            for config in websites {
                for i in 0..config.groupsize {
                    let wc = config.website_counter + i32::from(i);
                    let row = models::TaskInsert {
                        priority: prio + i32::from(i) + initial_priority,
                        name: &format!("{}-{}-{}", config.website, wc, config.groupid),
                        website: &config.website,
                        website_counter: wc,
                        state: models::TaskState::Created,
                        restart_count: 0,
                        last_modified: Utc::now(),
                        associated_data: None,
                        groupid: config.groupid,
                        groupsize: i32::from(config.groupsize),
                        uri: &config.uri,
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

    pub fn results_need_sanity_check_website(&self) -> Result<Option<Vec<models::Task>>, Error> {
        use diesel::dsl::sql_query;

        let conn = self.db_connection.lock().unwrap();
        let tasks = conn.transaction::<Vec<models::Task>, Error, _>(|| {
            Ok(sql_query(
                r#"SELECT
                t.id,
                t.priority,
                t.name,
                t.website,
                t.website_counter,
                t.state,
                t.restart_count,
                t.last_modified,
                t.associated_data,
                t.groupid,
                t.groupsize,
                t.uri
            FROM (
                SELECT website, groupid
                FROM tasks
                WHERE state = 'check_quality_domain'
                    AND aborted = false
                GROUP BY website, groupid
                HAVING count(*) = MAX(groupsize)
                LIMIT 1
            ) AS s
            JOIN tasks t
                ON s.website = t.website
               AND s.groupid = t.groupid

            ORDER BY
                t.website,
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

    pub fn mark_results_checked_website(&self, tasks: &mut Vec<models::Task>) -> Result<(), Error> {
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
            tasks[0].website(),
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

    pub fn restart_task(&self, task: &mut models::Task, reason: &dyn Display) -> Result<(), Error> {
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
            use crate::schema::tasks::dsl::{groupid, tasks, website};

            // We must abort all tasks for this website
            let msg = format!("Too many restarts for task {}, abort domain.", task.name());

            conn.transaction(|| {
                // get all tasks for the same website
                let other_tasks = tasks
                    .filter(website.eq(task.website()))
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

    pub fn restart_tasks(
        &self,
        tasks: &mut [models::Task],
        reason: &dyn Display,
    ) -> Result<(), Error> {
        // check that all tasks belong to the same website
        for task in &*tasks {
            assert_eq!(
                tasks[0].website(),
                task.website(),
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
            // We must abort all tasks for this website
            let msg = format!(
                "Too many restarts for domain {} groupid {}",
                tasks[0].website(),
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
        websites: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Vec<models::WebsiteCounters>, Error> {
        use diesel::{dsl::sql_query, sql_types::*};

        let conn = self.db_connection.lock().unwrap();
        let website_counters =
            conn.transaction::<Vec<models::WebsiteCounters>, Error, _>(|| {
                websites
                    .into_iter()
                    .map(|website| -> Result<models::WebsiteCounters, Error> {
                        let website = website.as_ref();
                        let res = sql_query(
                            r#"SELECT
                            website,
                            MAX(website_counter) + 1 as website_counter,
                            MAX(groupid) + 1 as groupid
                        FROM tasks
                        WHERE
                            website = $1
                        GROUP BY
                            website
                        ;"#,
                        )
                        .bind::<Text, _>(website)
                        .load::<models::WebsiteCounters>(&*conn)
                        .with_context(|| {
                            format!(
                                "Cannot retrieve website counters from database for website '{}'",
                                website,
                            )
                        })?;
                        // Check if there is a database result and if not, then create an artificial one
                        // Res is a Vec, but we want to move the first value out of the vec
                        Ok(res
                            .into_iter()
                            .next()
                            .unwrap_or_else(|| models::WebsiteCounters {
                                website: website.to_string(),
                                website_counter: 0,
                                groupid: 0,
                            }))
                    })
                    .collect()
            })?;

        Ok(website_counters)
    }
}

/// Configuration options for the Taskmanager
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
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub env: Environment,
}

impl Config {
    pub fn try_load_config(path: &Path) -> Result<Config, Error> {
        let content = read_to_string(path).context("Cannot read config file")?;
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConfig {
    pub remote_name: String,
    pub docker_image: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Environment {
    #[serde(flatten)]
    pub env: HashMap<String, String>,
}
