use serde::{Deserialize, Serialize};

use crate::database::projects;

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
pub struct Reset {
    pub project: String,
    pub deployment: Option<i32>,
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

#[derive(Serialize, Deserialize)]
pub struct LLMOutput {
    pub deployment: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Queue {
    pub deployment: i32,
}

#[derive(Serialize, Deserialize)]
pub struct PromoCodeRedeem {
    pub code: String,
}

#[derive(Serialize, Deserialize)]
pub struct PromoCode {
    pub code: String,
    pub credits: i64,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct PromoCodessAddition {
    pub promo_codes: String,
}
