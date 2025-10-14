use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, query, query_as, types::Json};

use crate::database::{Database, DatabaseConnection};

pub async fn create_table(connection: &DatabaseConnection) {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS projects(name TEXT PRIMARY KEY NOT NULL, owner TEXT NOT NULL, account_association JSON, base_build JSON, version TEXT)",
    )
    .execute(connection)
    .await
    .unwrap_or_else(|e| panic!("Could not create projects table: {e}"));
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountAssociation {
    pub header: String,
    pub payload: String,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BaseBuild {
    pub allowed_addresses: Vec<String>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct DatabaseProject {
    pub name: String,
    pub owner: String,
    pub account_association: Option<Json<AccountAssociation>>,
    pub base_build: Option<Json<BaseBuild>>,
    pub version: Option<String>,
}

impl DatabaseProject {
    #[allow(dead_code)]
    pub async fn get_all(database: &Database) -> Result<Vec<Self>, Error> {
        query_as("SELECT name, owner, account_association, base_build, version FROM projects")
            .fetch_all(&database.connection)
            .await
    }

    pub async fn get_all_by_owner(database: &Database, owner: &str) -> Result<Vec<Self>, Error> {
        query_as(
            "SELECT name, owner, account_association, base_build, version FROM projects WHERE owner = $1",
        )
        .bind(owner)
        .fetch_all(&database.connection)
        .await
    }

    pub async fn get_by_name(database: &Database, name: &str) -> Result<Option<Self>, Error> {
        query_as(
            "SELECT name, owner, account_association, base_build, version FROM projects WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&database.connection)
        .await
    }

    pub async fn insert(&self, database: &Database) -> Result<(), Error> {
        let Self {
            name,
            owner,
            account_association,
            base_build,
            version,
        } = self;

        query("INSERT INTO projects(name, owner, account_association, base_build, version) VALUES ($1, $2, $3, $4, $5);")
            .bind(name)
            .bind(owner)
            .bind(account_association)
            .bind(base_build)
            .bind(version)
            .execute(&database.connection)
            .await?;

        Ok(())
    }

    pub async fn update_account_association(
        &mut self,
        database: &Database,
        account_association: AccountAssociation,
    ) -> Result<(), Error> {
        let account_association = Json::from(account_association);
        query("UPDATE projects SET account_association = $1 WHERE name = $2;")
            .bind(&account_association)
            .bind(&self.name)
            .execute(&database.connection)
            .await?;

        self.account_association = Some(account_association);

        Ok(())
    }

    pub async fn update_base_build(
        &mut self,
        database: &Database,
        base_build: BaseBuild,
    ) -> Result<(), Error> {
        let base_build = Json::from(base_build);
        query("UPDATE projects SET base_build = $1 WHERE name = $2;")
            .bind(&base_build)
            .bind(&self.name)
            .execute(&database.connection)
            .await?;

        self.base_build = Some(base_build);

        Ok(())
    }

    pub fn get_flake(&self) -> String {
        let header = self
            .account_association
            .as_ref()
            .map(|json| json.header.clone())
            .unwrap_or_default();
        let payload = self
            .account_association
            .as_ref()
            .map(|json| json.payload.clone())
            .unwrap_or_default();
        let signature = self
            .account_association
            .as_ref()
            .map(|json| json.signature.clone())
            .unwrap_or_default();
        let allowed_addresses = self
            .base_build
            .as_ref()
            .map(|json| {
                json.allowed_addresses
                    .iter()
                    .map(|address| format!("\"{address}\""))
                    .collect::<Vec<String>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let version = self
            .version
            .as_ref()
            .map(|version| format!("/{version}"))
            .unwrap_or_default();
        format!(
            "\
{{
  inputs = {{
    xnode-manager.url = \"github:Openmesh-Network/xnode-manager\";
    xnode-miniapp-template.url = \"github:miniapp-factory/{name}{version}\";
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
          services.xnode-miniapp-template.accountAssociation = {{
            header = \"{header}\";
            payload = \"{payload}\";
            signature = \"{signature}\";
          }};
          services.xnode-miniapp-template.baseBuilder = {{
            allowedAddresses = [ {allowed_addresses} ];
          }};
        }}
      ];
    }};
  }};
}}", name = self.name
        )
    }
}
