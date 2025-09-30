use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub projects: Vec<String>,
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
