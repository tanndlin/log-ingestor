use tanlogs_client::TanLogsClient;

fn main() {
    let mut log_client = TanLogsClient::new("127.0.0.1:1337");
    for i in 0..10 {
        log_client.log_debug(format!("Test: {i}"));
    }
}
