use serde::Serialize;
use sqlx::{
    FromRow,
    types::chrono::{DateTime, Utc},
};
use util::{Level, Trace};

/// A row of `traces` along with the id used as the pagination tiebreaker.
#[derive(FromRow)]
pub struct TraceRow {
    pub id: i64,
    #[sqlx(flatten)]
    pub trace: Trace,
}

/// A [`Trace`] as returned to the viewer. Span ids are random 64-bit values,
/// which JSON numbers lose precision on in JS, so they go out as strings.
#[derive(Serialize)]
pub struct TraceResponse {
    id: i64,
    level: Level,
    timestamp: DateTime<Utc>,
    message: String,
    application: String,
    span_id: Option<String>,
}

impl From<TraceRow> for TraceResponse {
    fn from(TraceRow { id, trace }: TraceRow) -> Self {
        Self {
            id,
            level: trace.level,
            timestamp: trace.timestamp,
            message: trace.message,
            application: trace.application,
            span_id: trace.span_id.map(|id| id.to_string()),
        }
    }
}

/// Position of the last trace on a page; pass it back as
/// `cursor_timestamp`/`cursor_id` to get the page after it.
#[derive(Serialize)]
pub struct Cursor {
    timestamp: DateTime<Utc>,
    id: i64,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub traces: Vec<TraceResponse>,
    /// `None` once there are no more pages.
    pub next_cursor: Option<Cursor>,
}

impl SearchResponse {
    /// Builds a page from up to `limit + 1` rows; the extra row only signals
    /// that another page exists.
    pub fn from_rows(mut rows: Vec<TraceRow>, limit: usize) -> Self {
        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let next_cursor = has_more.then(|| rows.last()).flatten().map(|row| Cursor {
            timestamp: row.trace.timestamp,
            id: row.id,
        });
        Self {
            traces: rows.into_iter().map(TraceResponse::from).collect(),
            next_cursor,
        }
    }
}
