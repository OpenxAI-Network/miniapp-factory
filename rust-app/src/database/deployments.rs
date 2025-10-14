use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, query, query_as, query_scalar};

use crate::database::{Database, DatabaseConnection};

pub async fn create_table(connection: &DatabaseConnection) {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS deployments(id SERIAL PRIMARY KEY, project TEXT NOT NULL, instructions TEXT NOT NULL, submitted_at INT8 NOT NULL, coding_started_at INT8, coding_finished_at INT8, imagegen_started_at INT8, imagegen_finished_at INT8, git_hash TEXT, deployment_request INT8)",
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
    pub coding_started_at: Option<i64>,
    pub coding_finished_at: Option<i64>,
    pub imagegen_started_at: Option<i64>,
    pub imagegen_finished_at: Option<i64>,
    pub git_hash: Option<String>,
    pub deployment_request: Option<i64>,
}

impl DatabaseDeployment {
    #[allow(dead_code)]
    pub async fn get_all(database: &Database) -> Result<Vec<Self>, Error> {
        query_as("SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, imagegen_started_at, imagegen_finished_at, git_hash, deployment_request FROM deployments")
            .fetch_all(&database.connection)
            .await
    }

    pub async fn get_all_by_project(
        database: &Database,
        project: &str,
    ) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, imagegen_started_at, imagegen_finished_at, git_hash, deployment_request FROM deployments WHERE project = $1",
        )
        .bind(project)
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_next_unfinished(database: &Database) -> Result<Option<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, imagegen_started_at, imagegen_finished_at, git_hash, deployment_request FROM deployments WHERE coding_finished_at IS NULL ORDER BY id ASC LIMIT 1",
        )
        .fetch_optional(&database.connection)
        .await
    }

    pub async fn insert(&mut self, database: &Database) -> Result<(), Error> {
        let id: i32 = query_scalar("INSERT INTO deployments(project, instructions, submitted_at, coding_started_at, coding_finished_at, imagegen_started_at, imagegen_finished_at, git_hash, deployment_request) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id")
            .bind(&self.project)
            .bind(&self.instructions)
            .bind(self.submitted_at)
            .bind(self.coding_started_at)
            .bind(self.coding_finished_at)
            .bind(self.imagegen_started_at)
            .bind(self.imagegen_finished_at)
            .bind(&self.git_hash)
            .bind(self.deployment_request)
            .fetch_one(&database.connection)
            .await?;

        self.id = id;

        Ok(())
    }

    pub async fn update_coding_started_at(
        &mut self,
        database: &Database,
        coding_started_at: i64,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET coding_started_at = $1 WHERE id = $2;")
            .bind(coding_started_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.coding_started_at = Some(coding_started_at);

        Ok(())
    }

    pub async fn update_coding_finished_at(
        &mut self,
        database: &Database,
        coding_finished_at: i64,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET coding_finished_at = $1 WHERE id = $2;")
            .bind(coding_finished_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.coding_finished_at = Some(coding_finished_at);

        Ok(())
    }

    pub async fn update_imagegen_started_at(
        &mut self,
        database: &Database,
        imagegen_started_at: i64,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET imagegen_started_at = $1 WHERE id = $2;")
            .bind(imagegen_started_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.imagegen_started_at = Some(imagegen_started_at);

        Ok(())
    }

    pub async fn update_imagegen_finished_at(
        &mut self,
        database: &Database,
        imagegen_finished_at: i64,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET imagegen_finished_at = $1 WHERE id = $2;")
            .bind(imagegen_finished_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.imagegen_finished_at = Some(imagegen_finished_at);

        Ok(())
    }

    pub async fn update_git_hash(
        &mut self,
        database: &Database,
        git_hash: &str,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET git_hash = $1 WHERE id = $2;")
            .bind(git_hash)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.git_hash = Some(git_hash.to_string());

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
