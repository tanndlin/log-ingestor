use serde::{Deserialize, Serialize};
use sqlx::Type;

#[derive(Debug, Serialize, Deserialize, Type)]
#[sqlx(type_name = "level", rename_all = "UPPERCASE")]
pub enum Level {
    Fatal,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
