use crate::database::{Database, projects::DatabaseProject};

pub async fn get_price(database: &Database, user: &str) -> i64 {
    if let Ok(projects) = DatabaseProject::get_all_by_owner(database, user).await
        && (projects.is_empty())
        && let Ok(count) = DatabaseProject::get_count(database).await
        && count < 1000
    {
        return 0;
    }

    20_000_000
}
