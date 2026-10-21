#[tokio::main]
async fn main() {
    tcp_ingestor::run().await;
}
