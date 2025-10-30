use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PublicWaitlist {
    pub account: String,
    pub date: i64,
}
