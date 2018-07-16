#![recursion_limit = "128"]

extern crate chrono;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_derive_enum;
// #[macro_use]
extern crate failure;

use chrono::Local;
use diesel::{prelude::*, sqlite::SqliteConnection};
use failure::*;
use std::sync::Mutex;

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

    pub fn add_domain(&self, domain: &str, iteration_count: u8) -> Result<(), Error> {
        let conn = self.db_connection.lock().unwrap();
        for i in 0..iteration_count {
            let row = models::TaskInsert {
                priority: 0,
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
                .expect("Error creating new task");
        }
        Ok(())
    }
}
