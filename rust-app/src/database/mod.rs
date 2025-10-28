use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

use crate::utils::env::database;

pub mod coding_servers;
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
        .min_connections(10)
        .max_connections(10000)
        .test_before_acquire(false)
        .connect(&database())
        .await
        .unwrap_or_else(|e| panic!("Could not establish database connection: {e}"));

    coding_servers::create_table(&connection).await;
    deployments::create_table(&connection).await;
    projects::create_table(&connection).await;

    connection
}
