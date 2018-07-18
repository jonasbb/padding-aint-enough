#![recursion_limit = "128"]

extern crate chrono;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_derive_enum;
#[macro_use]
extern crate failure;
extern crate misc_utils;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use chrono::Local;
use diesel::{prelude::*, sqlite::SqliteConnection};
use failure::{Error, ResultExt};
use misc_utils::fs::file_open_read;
use std::{
    fmt::{self, Debug, Display},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub mod models;
pub mod schema;

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
);

#[derive(Clone)]
pub struct TaskManager {
    db_connection: Arc<Mutex<SqliteConnection>>,
}

impl Debug for TaskManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("TaskManager")
            .field("db_connection", &"<SqliteConnection>")
            .finish()
    }
}

impl TaskManager {
    pub fn new(database: &str) -> Result<Self, Error> {
        let db_connection = Arc::new(Mutex::new(SqliteConnection::establish(database)?));
        Ok(Self { db_connection })
    }

    pub fn delete_all(&self) -> Result<(), Error> {
        let conn = self.db_connection.lock().unwrap();
        conn.transaction::<(), _, _>(|| {
            diesel::delete(schema::tasks::table)
                .execute(&*conn)
                .context("Trying to delete `tasks` table")?;
            diesel::delete(schema::infos::table)
                .execute(&*conn)
                .context("Trying to delete `infos` table")?;
            Ok(())
        })
    }

    fn update_single_task(
        &self,
        conn: &SqliteConnection,
        task: &models::Task,
    ) -> Result<(), Error> {
        conn.transaction::<(), Error, _>(|| {
            diesel::update(task)
                .set(task)
                .execute(conn)
                .context("Cannot update task")?;
            Ok(())
        })
    }

    pub fn add_domains<I, S>(&self, domains: I, iteration_count: u8) -> Result<(), Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let conn = self.db_connection.lock().unwrap();
        conn.transaction::<(), _, _>(|| {
            for (prio, domain) in domains.into_iter().enumerate() {
                let prio = prio as i32;
                let domain = domain.as_ref();

                for i in 0..iteration_count {
                    let row = models::TaskInsert {
                        priority: prio * i32::from(iteration_count) + i32::from(i),
                        name: &format!("{}-{}", domain, i),
                        domain,
                        domain_counter: i32::from(i),
                        state: models::TaskState::Created,
                        restart_count: 0,
                        last_modified: Local::now().naive_local(),
                        associated_data: None,
                    };
                    diesel::insert_into(schema::tasks::table)
                        .values(&row)
                        .execute(&*conn)
                        .context("Error creating new task")?;
                }
            }
            Ok(())
        })
    }

    /// Return a task which waits for a VM to be executed
    pub fn get_task_for_vm(&self, executor: &Executor) -> Result<Option<models::Task>, Error> {
        use schema::tasks::dsl::{aborted, priority, state, tasks};

        let conn = self.db_connection.lock().unwrap();
        conn.transaction::<Option<models::Task>, _, _>(|| {
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
                task.associated_data =
                    Some(toml::to_string(&executor).context("Cannot serialize executor")?);
                diesel::update(&*task)
                    .set(&*task)
                    .execute(&*conn)
                    .context("Cannot update task")?;
            }
            Ok(task)
        })
    }

    pub fn finished_task_for_vm(
        &self,
        task: &mut models::Task,
        path_on_vm: &Path,
    ) -> Result<(), Error> {
        if task.state != models::TaskState::SubmittedToVm {
            bail!("To complete a VM task it must be in the SubmittedToVm state.")
        }

        // Update task state
        task.advance();
        let executor: Executor =
            toml::from_str(&*task.associated_data.as_ref().ok_or_else(|| {
                format_err!("Task in SubmittedToVm state must have associated data")
            })?).context("Associated data must be Executor")?;
        let new_data = ResultsCollectableData {
            executor,
            path_on_vm: path_on_vm.to_path_buf(),
        };
        task.associated_data = Some(
            toml::to_string(&new_data).context("Failed to serialize a ResultsCollectableData")?,
        );

        let conn = self.db_connection.lock().unwrap();
        self.update_single_task(&*conn, task)
    }

    pub fn results_collectable(
        &self,
    ) -> Result<Vec<(models::Task, ResultsCollectableData)>, Error> {
        use schema::tasks::dsl::{aborted, priority, state, tasks};

        let conn = self.db_connection.lock().unwrap();
        Ok(tasks
            .filter(state.eq(models::TaskState::ResultsCollectable))
            .filter(aborted.eq(false))
            .order_by(priority.asc())
            .select(TASKS_COLUMNS)
            .load::<models::Task>(&*conn)
            .context("Cannot retrieve tasks from database")?
            .into_iter()
            .map(|task| {
                let data: ResultsCollectableData =
                    toml::from_str(&*task.associated_data.as_ref().ok_or_else(|| {
                        format_err!("Task in ResultsCollectable state must have associated data")
                    })?).context("Associated data must be ResultsCollectableData")?;
                Ok((task, data))
            })
            .collect::<Result<Vec<_>, Error>>()?)
    }

    pub fn mark_results_collected(&self, task: &mut models::Task) -> Result<(), Error> {
        if task.state != models::TaskState::ResultsCollectable {
            bail!("Mark results as collected the task must be in the ResultsCollectable state.")
        }

        // Update task state
        task.advance();
        task.associated_data = None;

        let conn = self.db_connection.lock().unwrap();
        self.update_single_task(&*conn, task)
    }

    pub fn results_need_sanity_check_single(&self) -> Result<Vec<models::Task>, Error> {
        use schema::tasks::dsl::{aborted, priority, state, tasks};

        let conn = self.db_connection.lock().unwrap();
        Ok(tasks
            .filter(state.eq(models::TaskState::CheckQualitySingle))
            .filter(aborted.eq(false))
            .order_by(priority.asc())
            .select(TASKS_COLUMNS)
            .load::<models::Task>(&*conn)
            .context("Cannot retrieve tasks from database")?)
    }

    pub fn mark_results_checked_single(&self, task: &mut models::Task) -> Result<(), Error> {
        if task.state != models::TaskState::CheckQualitySingle {
            bail!("To check results they must be in the CheckQualitySingle state.")
        }

        // Update task state
        task.advance();
        task.associated_data = None;

        let conn = self.db_connection.lock().unwrap();
        self.update_single_task(&*conn, task)
    }

    pub fn results_need_sanity_check_domain(&self) -> Result<Vec<models::Task>, Error> {
        use schema::tasks::dsl::{aborted, priority, state, tasks};

        let conn = self.db_connection.lock().unwrap();
        Ok(tasks
            .filter(state.eq(models::TaskState::CheckQualityDomain))
            .filter(aborted.eq(false))
            .order_by(priority.asc())
            .select(TASKS_COLUMNS)
            .load::<models::Task>(&*conn)
            .context("Cannot retrieve tasks from database")?)
    }

    pub fn mark_results_checked_domain(&self, tasks: Vec<&mut models::Task>) -> Result<(), Error> {
        unimplemented!()
        // if task.state != models::TaskState::CheckQualitySingle {
        //     bail!("To check results they must be in the CheckQualitySingle state.")
        // }

        // // Update task state
        // task.advance();
        // task.associated_data = None;

        // self.update_single_task(task)
    }

    pub fn restart_task(&self, task: &mut models::Task, reason: &Display) -> Result<(), Error> {
        task.restart();
        task.associated_data = None;

        let conn = self.db_connection.lock().unwrap();

        if task.restart_count() < 4 {
            // The task is still allowed to be restarted
            let msg = format!("Restart task {} because {}", task.name(), reason);
            conn.transaction::<(), _, _>(|| {
                self.update_single_task(&*conn, task)?;

                let row = models::InfoInsert {
                    id: None,
                    task_id: task.id(),
                    time: Local::now().naive_local(),
                    message: &*msg,
                };
                diesel::insert_into(schema::infos::table)
                    .values(&row)
                    .execute(&*conn)
                    .context("Error creating new task")?;
                Ok(())
            })
        } else {
            use schema::tasks::dsl::{domain, tasks};

            // We must abort all tasks for this domain
            let msg = format!("Too many restarts for task {}, abort domain.", task.name());

            conn.transaction::<(), _, _>(|| {
                // get all tasks for the same domain
                let other_tasks = tasks
                    .filter(domain.eq(task.domain()))
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
                        time: Local::now().naive_local(),
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

    pub fn find_stale_tasks(&self) -> Result<Vec<models::Task>, Error> {
        unimplemented!()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultsCollectableData {
    pub path_on_vm: PathBuf,
    pub executor: Executor,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub working_directory: PathBuf,
    pub database: PathBuf,
    pub per_domain_datasets: u8,
    pub executors: Vec<Executor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Executor {
    pub name: String,
    pub sshconnect: String,
    pub working_directory: PathBuf,
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

    pub fn get_scripts_dir(&self) -> PathBuf {
        self.working_directory.join("scripts")
    }
}
