use std::sync::{Arc, Mutex};

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

pub async fn run(wal_writer: Arc<Mutex<WalWriter>>) {
    let port = std::env::var("HTTP_PORT").unwrap_or("1338".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    println!(
        "HTTP Ingestor listening on: {}",
        listener.local_addr().unwrap()
    );

    let app = Router::new()
        .route("/traces", post(ingest))
        .with_state(wal_writer);

    axum::serve(listener, app).await.unwrap();
}

async fn ingest(
    State(wal_writer): State<Arc<Mutex<WalWriter>>>,
    Json(TraceIngest { level, message }): Json<TraceIngest>,
) -> StatusCode {
    let trace = Trace::new(level, message);

    wal_writer.lock().unwrap().write(&trace);
    StatusCode::ACCEPTED
}
