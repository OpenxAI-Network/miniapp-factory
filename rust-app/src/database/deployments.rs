use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, query, query_as, query_scalar};

use crate::database::{Database, DatabaseConnection};

pub async fn create_table(connection: &DatabaseConnection) {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS deployments(id SERIAL PRIMARY KEY, project TEXT NOT NULL, instructions TEXT NOT NULL, submitted_at INT8 NOT NULL, coding_started_at INT8, coding_finished_at INT8, coding_git_hash TEXT, imagegen_started_at INT8, imagegen_finished_at INT8, imagegen_git_hash TEXT, deployment_request INT8, deleted BOOL NOT NULL)",
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
    pub coding_git_hash: Option<String>,
    pub imagegen_started_at: Option<i64>,
    pub imagegen_finished_at: Option<i64>,
    pub imagegen_git_hash: Option<String>,
    pub deployment_request: Option<i64>,
    pub deleted: bool,
}

impl DatabaseDeployment {
    #[allow(dead_code)]
    pub async fn get_all(database: &Database) -> Result<Vec<Self>, Error> {
        query_as("SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, coding_git_hash, imagegen_started_at, imagegen_finished_at, imagegen_git_hash, deployment_request, deleted FROM deployments")
            .fetch_all(&database.connection)
            .await
    }

    #[allow(dead_code)]
    pub async fn get_all_by_project(
        database: &Database,
        project: &str,
    ) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, coding_git_hash, imagegen_started_at, imagegen_finished_at, imagegen_git_hash, deployment_request, deleted FROM deployments WHERE project = $1",
        )
        .bind(project)
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_all_by_project_undeleted(
        database: &Database,
        project: &str,
    ) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, coding_git_hash, imagegen_started_at, imagegen_finished_at, imagegen_git_hash, deployment_request, deleted FROM deployments WHERE project = $1 and deleted = FALSE",
        )
        .bind(project)
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_all_by_project_unfinished(
        database: &Database,
        project: &str,
    ) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, coding_git_hash, imagegen_started_at, imagegen_finished_at, imagegen_git_hash, deployment_request, deleted FROM deployments WHERE coding_started_at IS NULL AND project = $1",
        )
        .bind(project)
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_next_unfinished(database: &Database) -> Result<Option<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, coding_git_hash, imagegen_started_at, imagegen_finished_at, imagegen_git_hash, deployment_request, deleted FROM deployments WHERE coding_started_at IS NULL ORDER BY id ASC LIMIT 1",
        )
        .fetch_optional(&database.connection)
        .await
    }

    pub async fn get_queued_count(database: &Database) -> Result<i64, Error> {
        query_scalar("SELECT COUNT(id) FROM deployments WHERE coding_started_at IS NULL")
            .fetch_one(&database.connection)
            .await
    }

    pub async fn get_queued_count_before(database: &Database, before: i32) -> Result<i64, Error> {
        query_scalar(
            "SELECT COUNT(id) FROM deployments WHERE coding_started_at IS NULL AND id < $1",
        )
        .bind(before)
        .fetch_one(&database.connection)
        .await
    }

    pub async fn get_by_id(database: &Database, id: i32) -> Result<Option<Self>, Error> {
        query_as(
            "SELECT id, project, instructions, submitted_at, coding_started_at, coding_finished_at, coding_git_hash, imagegen_started_at, imagegen_finished_at, imagegen_git_hash, deployment_request, deleted FROM deployments WHERE id = $1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&database.connection)
        .await
    }

    pub async fn delete_all_after(
        database: &Database,
        project: &str,
        after: i32,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET deleted = TRUE WHERE project = $1 AND id > $2;")
            .bind(project)
            .bind(after)
            .execute(&database.connection)
            .await?;

        Ok(())
    }

    pub async fn insert(&mut self, database: &Database) -> Result<(), Error> {
        let id: i32 = query_scalar("INSERT INTO deployments(project, instructions, submitted_at, coding_started_at, coding_finished_at, coding_git_hash, imagegen_started_at, imagegen_finished_at, imagegen_git_hash, deployment_request, deleted) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id")
            .bind(&self.project)
            .bind(&self.instructions)
            .bind(self.submitted_at)
            .bind(self.coding_started_at)
            .bind(self.coding_finished_at)
            .bind(&self.coding_git_hash)
            .bind(self.imagegen_started_at)
            .bind(self.imagegen_finished_at)
            .bind(&self.imagegen_git_hash)
            .bind(self.deployment_request)
            .bind(self.deleted)
            .fetch_one(&database.connection)
            .await?;

        self.id = id;

        Ok(())
    }

    pub async fn update_coding_started_at(
        &mut self,
        database: &Database,
        coding_started_at: Option<i64>,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET coding_started_at = $1 WHERE id = $2;")
            .bind(coding_started_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.coding_started_at = coding_started_at;

        Ok(())
    }

    pub async fn update_coding_finished_at(
        &mut self,
        database: &Database,
        coding_finished_at: Option<i64>,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET coding_finished_at = $1 WHERE id = $2;")
            .bind(coding_finished_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.coding_finished_at = coding_finished_at;

        Ok(())
    }

    pub async fn update_coding_git_hash(
        &mut self,
        database: &Database,
        coding_git_hash: Option<String>,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET coding_git_hash = $1 WHERE id = $2;")
            .bind(&coding_git_hash)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.coding_git_hash = coding_git_hash;

        Ok(())
    }

    pub async fn update_imagegen_started_at(
        &mut self,
        database: &Database,
        imagegen_started_at: Option<i64>,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET imagegen_started_at = $1 WHERE id = $2;")
            .bind(imagegen_started_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.imagegen_started_at = imagegen_started_at;

        Ok(())
    }

    pub async fn update_imagegen_finished_at(
        &mut self,
        database: &Database,
        imagegen_finished_at: Option<i64>,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET imagegen_finished_at = $1 WHERE id = $2;")
            .bind(imagegen_finished_at)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.imagegen_finished_at = imagegen_finished_at;

        Ok(())
    }

    pub async fn update_imagegen_git_hash(
        &mut self,
        database: &Database,
        imagegen_git_hash: Option<String>,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET imagegen_git_hash = $1 WHERE id = $2;")
            .bind(&imagegen_git_hash)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.imagegen_git_hash = imagegen_git_hash;

        Ok(())
    }

    pub async fn update_deployment_request(
        &mut self,
        database: &Database,
        deployment_request: Option<i64>,
    ) -> Result<(), Error> {
        query("UPDATE deployments SET deployment_request = $1 WHERE id = $2;")
            .bind(deployment_request)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.deployment_request = deployment_request;

        Ok(())
    }
}
