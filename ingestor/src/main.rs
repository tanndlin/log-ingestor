use std::sync::{Arc, Mutex};

use writer::get_writers;

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let (wal_writer, _db_writer) = get_writers("wal", &db_url).unwrap();
    let wal_writer = Arc::new(Mutex::new(wal_writer));

    tcp_ingestor::run(wal_writer).await;
}
