mod search_query;
mod trace_response;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use sqlx::{PgPool, QueryBuilder};
use tokio::net::TcpListener;

use crate::{
    search_query::SearchQuery,
    trace_response::{SearchResponse, TraceRow},
};

const DEFAULT_PAGE_SIZE: u16 = 100;
const MAX_PAGE_SIZE: u16 = 1000;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL needs to be set")?;
    let pool = PgPool::connect(&db_url).await.context("connecting to db")?;
    let state = AppState { pool };

    let app = Router::new()
        .route("/health", get(|| async { "<h1>Hello World!</h1>" }))
        .route("/search", get(search))
        .with_state(state);

    let port = std::env::var("QUERIER_PORT").unwrap_or("1336".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| format!("binding querier to port {port}"))?;

    let local_addr = listener.local_addr()?;
    println!("Log Querier listening on {local_addr}");
    axum::serve(listener, app).await.context("serving querier")
}

async fn search(
    Query(params): Query<SearchQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut qb = QueryBuilder::new("SELECT * FROM traces WHERE 1=1");
    if let Some(level) = params.level {
        qb.push(" AND level = ").push_bind(level);
    }
    if let Some(message) = params.message {
        qb.push(" AND message ILIKE ")
            .push_bind(format!("%{message}%"));
    }
    if let Some(before) = params.before {
        qb.push(" AND timestamp < ").push_bind(before);
    }
    if let Some(after) = params.after {
        qb.push(" AND timestamp > ").push_bind(after);
    }
    if let Some(application) = params.application {
        qb.push(" AND application ILIKE ")
            .push_bind(format!("%{application}%"));
    }
    if let Some(span_id) = params.span_id {
        qb.push(" AND span_id = ").push_bind(span_id);
    }
    if let (Some(ts), Some(id)) = (params.cursor_timestamp, params.cursor_id) {
        qb.push(" AND (timestamp, id) < (")
            .push_bind(ts)
            .push(", ")
            .push_bind(id)
            .push(")");
    }

    let limit = params
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    // One extra row tells us whether there's another page.
    qb.push(" ORDER BY timestamp DESC, id DESC LIMIT ")
        .push_bind(i64::from(limit) + 1);

    qb.build_query_as::<TraceRow>()
        .fetch_all(&state.pool)
        .await
        .map(|rows| Json(SearchResponse::from_rows(rows, usize::from(limit))))
        .map_err(|e| {
            eprintln!("Search query failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
