use std::thread;

fn main() {
    let http = thread::spawn(http_ingestor::run);
    let protobuf = thread::spawn(protobuf_ingestor::run);

    http.join().expect("HTTP ingestor panicked");
    protobuf.join().expect("protobuf ingestor panicked");
}
