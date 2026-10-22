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
}

impl Trace {
    #[must_use]
    pub fn new(level: Level, message: String) -> Self {
        Self {
            level,
            timestamp: Utc::now(),
            message,
        }
    }

    #[must_use]
    pub fn to_vec(&self) -> Vec<u8, MAX_TRACE_SIZE> {
        to_vec(self).unwrap()
    }
}
