use serde::{Deserialize, Serialize};

use crate::database::projects;

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub projects: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Available {
    pub project: String,
}

#[derive(Serialize, Deserialize)]
pub struct Create {
    pub project: String,
}

#[derive(Serialize, Deserialize)]
pub struct Change {
    pub project: String,
    pub instructions: String,
}

#[derive(Serialize, Deserialize)]
pub struct History {
    pub project: String,
}

#[derive(Serialize, Deserialize)]
pub struct AccountAssociation {
    pub project: String,
    pub account_association: projects::AccountAssociation,
}

#[derive(Serialize, Deserialize)]
pub struct BaseBuild {
    pub project: String,
    pub base_build: projects::BaseBuild,
}
