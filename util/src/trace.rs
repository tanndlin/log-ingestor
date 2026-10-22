use chrono::{DateTime, Utc};
use heapless::Vec;
use postcard::to_vec;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::level::Level;

pub const MAX_TRACE_SIZE: usize = 1024;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Trace {
    pub level: Level,
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub application: String,
    pub span_id: Option<i64>,
}

impl Trace {
    #[must_use]
    pub fn new(level: Level, message: String, application: String, span_id: Option<i64>) -> Self {
        Self {
            level,
            timestamp: Utc::now(),
            message,
            application,
            span_id,
        }
    }

    pub fn to_vec(&self) -> Result<Vec<u8, MAX_TRACE_SIZE>, postcard::Error> {
        to_vec(self)
    }
}
