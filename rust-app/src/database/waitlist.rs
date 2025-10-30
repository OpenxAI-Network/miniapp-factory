use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, query_as, query_scalar};

use crate::database::{Database, DatabaseConnection};

pub async fn create_table(connection: &DatabaseConnection) {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS waitlist(id SERIAL PRIMARY KEY, account TEXT NOT NULL, ip TEXT UNIQUE NOT NULL, date INT8 NOT NULL)",
    )
    .execute(connection)
    .await
    .unwrap_or_else(|e| panic!("Could not create projects table: {e}"));
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct DatabaseWaitlist {
    pub id: i32,
    pub account: String,
    pub ip: String,
    pub date: i64,
}

impl DatabaseWaitlist {
    pub async fn get_all(database: &Database) -> Result<Vec<Self>, Error> {
        query_as("SELECT id, account, ip, date FROM waitlist ORDER BY id")
            .fetch_all(&database.connection)
            .await
    }

    pub async fn get_by_account(database: &Database, account: &str) -> Result<Option<Self>, Error> {
        query_as("SELECT id, account, ip, date FROM waitlist WHERE account = $1")
            .bind(account)
            .fetch_optional(&database.connection)
            .await
    }

    #[allow(dead_code)]
    pub async fn get_by_ip(database: &Database, ip: &str) -> Result<Option<Self>, Error> {
        query_as("SELECT id, account, ip, date FROM waitlist WHERE ip = $1")
            .bind(ip)
            .fetch_optional(&database.connection)
            .await
    }

    pub async fn insert(&mut self, database: &Database) -> Result<(), Error> {
        let id: i32 = query_scalar(
            "INSERT INTO waitlist(account, ip, date) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(&self.account)
        .bind(&self.ip)
        .bind(&self.date)
        .fetch_one(&database.connection)
        .await?;

        self.id = id;

        Ok(())
    }
}
