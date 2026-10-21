mod search_query;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use sqlx::{PgPool, QueryBuilder};
use tokio::net::TcpListener;
use util::Trace;

use crate::search_query::SearchQuery;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL needs to be set");
    let pool = PgPool::connect(&db_url).await.unwrap();
    let state = AppState { pool };

    let app = Router::new()
        .route("/health", get(|| async { "<h1>Hello World!</h1>" }))
        .route("/search", get(search))
        .with_state(state);

    let port = std::env::var("QUERIER_PORT").unwrap_or("1336".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();

    let local_addr = listener.local_addr().unwrap();
    println!("Log Querier listening on {local_addr}");
    axum::serve(listener, app).await.unwrap();
}

async fn search(
    Query(params): Query<SearchQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut qb = QueryBuilder::new("SELECT level, timestamp, message FROM traces WHERE 1=1");
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

    qb.build_query_as::<Trace>()
        .fetch_all(&state.pool)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
