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
    pub fn debug(message: String) -> Self {
        Self {
            level: Level::Debug,
            timestamp: Utc::now(),
            message,
        }
    }

    pub fn to_vec(&self) -> Vec<u8, MAX_TRACE_SIZE> {
        to_vec(self).unwrap()
    }
}
