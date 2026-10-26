use serde::Deserialize;
use sqlx::types::chrono::{DateTime, Utc};
use util::Level;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub level: Option<Level>,
    pub message: Option<String>,
    pub before: Option<DateTime<Utc>>,
    pub after: Option<DateTime<Utc>>,
    pub application: Option<String>,
    pub span_id: Option<i64>,
    pub limit: Option<u16>,
    /// Keyset cursor from a previous page's `next_cursor`. Both halves are
    /// needed: timestamps alone aren't unique.
    pub cursor_timestamp: Option<DateTime<Utc>>,
    pub cursor_id: Option<i64>,
}
