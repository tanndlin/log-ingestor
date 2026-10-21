use std::sync::{Arc, Mutex};

use writer::get_writers;

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let (wal_writer, _db_writer) = get_writers("wal", &db_url).unwrap();
    let wal_writer = Arc::new(Mutex::new(wal_writer));

    tokio::select! {
        () = tcp_ingestor::run(wal_writer) => {}
        () = shutdown_signal() => println!("Shutting down"),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.unwrap();
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .unwrap()
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
