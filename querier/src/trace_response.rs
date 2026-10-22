use serde::Serialize;
use sqlx::types::chrono::{DateTime, Utc};
use util::{Level, Trace};

/// A [`Trace`] as returned to the viewer. Span ids are random 64-bit values,
/// which JSON numbers lose precision on in JS, so they go out as strings.
#[derive(Serialize)]
pub struct TraceResponse {
    level: Level,
    timestamp: DateTime<Utc>,
    message: String,
    application: String,
    span_id: Option<String>,
}

impl From<Trace> for TraceResponse {
    fn from(trace: Trace) -> Self {
        Self {
            level: trace.level,
            timestamp: trace.timestamp,
            message: trace.message,
            application: trace.application,
            span_id: trace.span_id.map(|id| id.to_string()),
        }
    }
}
