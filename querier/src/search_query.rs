use serde::Deserialize;
use sqlx::types::chrono::{DateTime, Utc};
use util::Level;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub level: Option<Level>,
    pub message: Option<String>,
    pub before: Option<DateTime<Utc>>,
    pub after: Option<DateTime<Utc>>,
}
