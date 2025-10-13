use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, query, query_as, query_scalar};

use crate::database::{Database, DatabaseConnection};

pub async fn create_table(connection: &DatabaseConnection) {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS deployments(id SERIAL PRIMARY KEY, project TEXT NOT NULL, instructions TEXT NOT NULL, submitted_at INT8 NOT NULL, started_at INT8, finished_at INT8, deployment_request INT8)",
    )
    .execute(connection)
    .await
    .unwrap_or_else(|e| panic!("Could not create projects table: {e}"));
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct DatabaseDeployment {
    pub id: i32,
    pub project: String,
    pub instructions: String,
    pub submitted_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub deployment_request: Option<i64>,
}

impl DatabaseDeployment {
    #[allow(dead_code)]
    pub async fn get_all(database: &Database) -> Result<Vec<Self>, Error> {
        query_as("SELECT id, project, instructions, submitted_at, started_at, finished_at, deployment_request FROM deployments")
            .fetch_all(&database.connection)
            .await
    }

    pub async fn get_all_by_project(
        database: &Database,
        project: &str,
    ) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, started_at, finished_at, deployment_request FROM deployments WHERE project = $1",
        )
        .bind(project)
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_next_unfinished(database: &Database) -> Result<Option<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, started_at, finished_at, deployment_request FROM deployments WHERE finished_at IS NULL ORDER BY id ASC LIMIT 1",
        )
        .fetch_optional(&database.connection)
        .await
    }

    pub async fn insert(&mut self, database: &Database) -> Result<(), Error> {
        let id: i32 = query_scalar("INSERT INTO deployments(project, instructions, submitted_at, started_at, finished_at, deployment_request) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id")
            .bind(&self.project)
            .bind(&self.instructions)
            .bind(self.submitted_at)
            .bind(self.started_at)
            .bind(self.finished_at)
            .bind(self.deployment_request)
            .fetch_one(&database.connection)
            .await?;

        self.id = id;

        Ok(())
    }

    pub async fn update_started_at(
        &mut self,
        database: &Database,
        started_at: i64,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET started_at = $1 WHERE id = $2;")
            .bind(started_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.started_at = Some(started_at);

        Ok(())
    }

    pub async fn update_finished_at(
        &mut self,
        database: &Database,
        finished_at: i64,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET finished_at = $1 WHERE id = $2;")
            .bind(finished_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.finished_at = Some(finished_at);

        Ok(())
    }

    pub async fn update_deployment_request(
        &mut self,
        database: &Database,
        deployment_request: i64,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET deployment_request = $1 WHERE id = $2;")
            .bind(deployment_request)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.deployment_request = Some(deployment_request);

        Ok(())
    }
}
