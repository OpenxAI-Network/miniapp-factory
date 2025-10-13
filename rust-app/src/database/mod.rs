use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

use crate::utils::env::database;

pub mod deployments;
pub mod projects;

pub type DatabaseConnection = Pool<Postgres>;

#[derive(Clone)]
pub struct Database {
    connection: DatabaseConnection,
}

impl Database {
    pub async fn new() -> Self {
        Self {
            connection: create_connection().await,
        }
    }
}

pub async fn create_connection() -> DatabaseConnection {
    let connection = PgPoolOptions::new()
        .max_connections(10000)
        .connect(&database())
        .await
        .unwrap_or_else(|e| panic!("Could not establish database connection: {e}"));

    projects::create_table(&connection).await;
    deployments::create_table(&connection).await;

    connection
}
