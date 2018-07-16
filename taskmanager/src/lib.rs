#![recursion_limit = "128"]

extern crate chrono;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_derive_enum;
// #[macro_use]
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
    path::{Path, PathBuf},
    sync::Mutex,
};

pub mod models;
pub mod schema;

pub struct TaskManager {
    db_connection: Mutex<SqliteConnection>,
}

impl TaskManager {
    pub fn new(database: &str) -> Result<Self, Error> {
        let db_connection = Mutex::new(SqliteConnection::establish(database)?);
        Ok(Self { db_connection })
    }

    pub fn delete_all_tasks(&self) -> Result<(), Error> {
        let conn = self.db_connection.lock().unwrap();
        conn.transaction::<(), _, _>(|| {
            diesel::delete(schema::tasks::table)
                .execute(&*conn)
                .context("Trying to delete `tasks` table")?;
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
        use schema::tasks::dsl::{priority, state, tasks};

        let conn = self.db_connection.lock().unwrap();
        conn.transaction::<Option<models::Task>, _, _>(|| {
            let res = tasks
                .filter(state.eq(models::TaskState::Created))
                .order_by(priority.asc())
                .limit(1)
                .load::<models::Task>(&*conn)
                .context("Cannot retrieve task from database")?;

            // we only fetch one task, so this next is sufficient to retrieve all data
            let mut task = res.into_iter().next();
            if let Some(ref mut task) = &mut task {
                eprintln!("{:?}", task);
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub working_directory: PathBuf,
    pub database: PathBuf,
    pub per_domain_datasets: u8,
    pub executors: Vec<Executor>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Executor {
    name: String,
    sshconnect: Vec<String>,
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
}
