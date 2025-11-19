use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ProjectShowcase {
    pub id: i32,
    pub name: String,
}
