use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, query, query_as, query_scalar, types::Json};
use xnode_deployer::hyperstack::HyperstackOutput;

use crate::database::{Database, DatabaseConnection};

pub async fn create_table(connection: &DatabaseConnection) {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS coding_servers(id SERIAL PRIMARY KEY, hardware JSON NOT NULL, container_deployment INT8, setup_finished BOOL NOT NULL, assignment INT4, dynamic BOOL NOT NULL)",
    )
    .execute(connection)
    .await
    .unwrap_or_else(|e| panic!("Could not create projects table: {e}"));
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct DatabaseCodingServer {
    pub id: i32,
    pub hardware: Json<HyperstackOutput>,
    pub container_deployment: Option<i64>,
    pub setup_finished: bool,
    pub assignment: Option<i32>,
    pub dynamic: bool,
}

impl DatabaseCodingServer {
    #[allow(dead_code)]
    pub async fn get_all(database: &Database) -> Result<Vec<Self>, Error> {
        query_as("SELECT id, hardware, container_deployment, setup_finished, assignment, dynamic FROM coding_servers")
            .fetch_all(&database.connection)
            .await
    }

    pub async fn get_count(database: &Database) -> Result<i64, Error> {
        query_scalar("SELECT COUNT(id) FROM coding_servers")
            .fetch_one(&database.connection)
            .await
    }

    pub async fn get_all_no_setup_finished(database: &Database) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT id, hardware, container_deployment, setup_finished, assignment, dynamic FROM coding_servers WHERE setup_finished = FALSE",
        )
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_all_dynamic_unassigned(database: &Database) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT id, hardware, container_deployment, setup_finished, assignment, dynamic FROM coding_servers WHERE dynamic = TRUE AND assignment IS NULL",
        )
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_all_assigned(database: &Database) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT id, hardware, container_deployment, setup_finished, assignment, dynamic FROM coding_servers WHERE assignment IS NOT NULL",
        )
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_available(database: &Database) -> Result<Option<Self>, Error> {
        query_as(
            "SELECT id, hardware, container_deployment, setup_finished, assignment, dynamic FROM coding_servers WHERE setup_finished = TRUE AND assignment IS NULL LIMIT 1",
        )
        .fetch_optional(&database.connection)
        .await
    }

    pub async fn get_by_assignment(
        database: &Database,
        assignment: Option<i32>,
    ) -> Result<Option<Self>, Error> {
        query_as(
            "SELECT id, hardware, container_deployment, setup_finished, assignment, dynamic FROM coding_servers WHERE assignment = $1 LIMIT 1",
        ).bind(assignment)
        .fetch_optional(&database.connection)
        .await
    }

    pub async fn insert(&mut self, database: &Database) -> Result<(), Error> {
        let id: i32 = query_scalar("INSERT INTO coding_servers(hardware, container_deployment, setup_finished, assignment, dynamic) VALUES ($1, $2, $3, $4, $5) RETURNING id")
            .bind(&self.hardware)
            .bind(self.container_deployment)
            .bind(self.setup_finished)
            .bind(self.assignment)
            .bind(self.dynamic)
            .fetch_one(&database.connection)
            .await?;

        self.id = id;

        Ok(())
    }

    pub async fn update_container_deployment(
        &mut self,
        database: &Database,
        container_deployment: Option<i64>,
    ) -> Result<(), Error> {
        query("UPDATE coding_servers SET container_deployment = $1 WHERE id = $2;")
            .bind(container_deployment)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.container_deployment = container_deployment;

        Ok(())
    }

    pub async fn update_setup_finished(
        &mut self,
        database: &Database,
        setup_finished: bool,
    ) -> Result<(), Error> {
        query("UPDATE coding_servers SET setup_finished = $1 WHERE id = $2;")
            .bind(setup_finished)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.setup_finished = setup_finished;

        Ok(())
    }

    pub async fn update_assignment(
        &mut self,
        database: &Database,
        assignment: Option<i32>,
    ) -> Result<(), Error> {
        query("UPDATE coding_servers SET assignment = $1 WHERE id = $2;")
            .bind(assignment)
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        self.assignment = assignment;

        Ok(())
    }

    pub async fn delete(self, database: &Database) -> Result<(), Error> {
        query("DELETE FROM coding_servers WHERE id = $1;")
            .bind(self.id)
            .execute(&database.connection)
            .await?;

        Ok(())
    }
}
