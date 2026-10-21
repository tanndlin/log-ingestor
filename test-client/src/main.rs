use tanlogs_client::TanLogsClient;

fn main() {
    let mut log_client = TanLogsClient::new("127.0.0.1:1337");
    for i in 0..10u32 {
        if i.is_multiple_of(2) {
            log_client.debug(format!("Test: {i}"));
        } else {
            log_client.info(format!("Test: {i}"));
        }
    }
}
