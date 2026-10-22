mod counting_reader;
mod db_writer;
mod event;
mod wal_pos;
mod wal_writer;

use std::{path::PathBuf, sync::mpsc, thread::JoinHandle};

use anyhow::Result;
pub use db_writer::DbWriter;
pub use wal_writer::WalWriter;

pub fn get_writers(
    wal_dir: impl Into<PathBuf>,
    db_url: &str,
) -> Result<(WalWriter, JoinHandle<()>)> {
    let (tx, rx) = mpsc::channel();
    let wal_dir = wal_dir.into();

    let wal_writer = WalWriter::new(wal_dir.clone(), tx)?;
    let db_writer = db_writer::create_db_writer(wal_dir, db_url, rx)?;

    Ok((wal_writer, db_writer))
}
