use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};

use writer::get_writers;

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let (wal_writer, _db_writer) = get_writers("wal", &db_url)?;
    let wal_writer = Arc::new(Mutex::new(wal_writer));

    tokio::select! {
        r = tcp_ingestor::run(wal_writer.clone()) => r?,
        r = http_ingestor::run(wal_writer) => r?,
        r = shutdown_signal() => {
            r?;
            println!("Shutting down");
        }
    }
    Ok(())
}

async fn shutdown_signal() -> Result<()> {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .context("listening for Ctrl+C")
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .context("listening for SIGTERM")?
            .recv()
            .await;
        Ok::<_, anyhow::Error>(())
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<Result<()>>();

    tokio::select! {
        r = ctrl_c => r,
        r = terminate => r,
    }
}
