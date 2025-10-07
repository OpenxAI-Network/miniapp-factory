use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, query, query_as, types::Json};

use crate::database::{Database, DatabaseConnection};

pub async fn create_table(connection: &DatabaseConnection) {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS projects(name TEXT PRIMARY KEY NOT NULL, owner TEXT NOT NULL, account_association JSON)",
    )
    .execute(connection)
    .await
    .unwrap_or_else(|e| panic!("Could not create projects table: {e}"));
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountAssociation {
    header: String,
    payload: String,
    signature: String,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct DatabaseProject {
    pub name: String,
    pub owner: String,
    pub account_association: Option<Json<AccountAssociation>>,
}

impl DatabaseProject {
    #[allow(dead_code)]
    pub async fn get_all(database: &Database) -> Result<Vec<Self>, Error> {
        query_as("SELECT name, owner, account_association FROM projects")
            .fetch_all(&database.connection)
            .await
    }

    pub async fn get_all_by_owner(database: &Database, owner: &str) -> Result<Vec<Self>, Error> {
        query_as("SELECT name, owner, account_association FROM projects WHERE owner = $1")
            .bind(owner)
            .fetch_all(&database.connection)
            .await
    }

    pub async fn get_by_name(database: &Database, name: &str) -> Result<Option<Self>, Error> {
        query_as("SELECT name, owner, account_association FROM projects WHERE name = $1")
            .bind(name)
            .fetch_optional(&database.connection)
            .await
    }

    pub async fn insert(&self, database: &Database) -> Result<(), Error> {
        let Self {
            name,
            owner,
            account_association,
        } = self;

        query("INSERT INTO projects(name, owner, account_association) VALUES ($1, $2, $3);")
            .bind(name)
            .bind(owner)
            .bind(account_association)
            .execute(&database.connection)
            .await?;

        Ok(())
    }

    pub fn get_flake(&self) -> String {
        format!(
            "\
{{
  inputs = {{
    xnode-manager.url = \"github:Openmesh-Network/xnode-manager\";
    xnode-miniapp-template.url = \"github:miniapp-factory/{name}\";
    nixpkgs.follows = \"xnode-miniapp-template/nixpkgs\";
  }};

  outputs = inputs: {{
    nixosConfigurations.container = inputs.nixpkgs.lib.nixosSystem {{
      specialArgs = {{
        inherit inputs;
      }};
      modules = [
        inputs.xnode-manager.nixosModules.container
        {{
          services.xnode-container.xnode-config = {{
            host-platform = ./xnode-config/host-platform;
            state-version = ./xnode-config/state-version;
            hostname = ./xnode-config/hostname;
          }};
        }}
        inputs.xnode-miniapp-template.nixosModules.default
        {{
          services.xnode-miniapp-template.enable = true;
          services.xnode-miniapp-template.url = \"https://{name}.miniapp-factory.marketplace.openxai.network\";
        }}
      ];
    }};
  }};
}}", name = self.name
        )
    }
}
