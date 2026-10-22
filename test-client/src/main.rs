use anyhow::Result;
use tanlogs_client::TanLogsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(TanLogsLayer::new("127.0.0.1:1337", "test-client")?)
        .init();

    for i in 0..10u32 {
        if i.is_multiple_of(2) {
            tracing::debug!("Test: {i}");
        } else {
            tracing::info!("Test: {i}");
        }
    }
    Ok(())
}
