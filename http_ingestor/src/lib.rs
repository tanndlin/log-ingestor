use std::sync::{Arc, Mutex, PoisonError};

use anyhow::{Context, Result};
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use serde::Deserialize;
use tokio::net::TcpListener;
use util::{Level, Trace};
use writer::WalWriter;

#[derive(Deserialize)]
struct TraceIngest {
    level: Level,
    message: String,
}

pub async fn run(wal_writer: Arc<Mutex<WalWriter>>) -> Result<()> {
    let port = std::env::var("HTTP_PORT").unwrap_or("1338".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| format!("binding HTTP ingestor to port {port}"))?;
    println!("HTTP Ingestor listening on: {}", listener.local_addr()?);

    let app = Router::new()
        .route("/traces", post(ingest))
        .with_state(wal_writer);

    axum::serve(listener, app)
        .await
        .context("serving HTTP ingestor")
}

async fn ingest(
    State(wal_writer): State<Arc<Mutex<WalWriter>>>,
    Json(TraceIngest { level, message }): Json<TraceIngest>,
) -> StatusCode {
    let trace = Trace::new(level, message);

    // WalWriter::write has no panic paths, so a poisoned lock leaves it usable
    let mut wal_writer = wal_writer.lock().unwrap_or_else(PoisonError::into_inner);
    if let Err(e) = wal_writer.write(&trace) {
        eprintln!("WAL write failed: {e:#}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::ACCEPTED
}
