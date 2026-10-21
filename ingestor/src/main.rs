use std::sync::{Arc, Mutex};

use writer::WalWriter;

#[tokio::main]
async fn main() {
    let wal_writer = Arc::new(Mutex::new(WalWriter::new("wal")));
    tcp_ingestor::run(wal_writer).await;
}
