use std::{
    io::Write,
    net::{TcpStream, ToSocketAddrs},
};

use util::Trace;

pub struct TanLogsClient {
    tcp_stream: TcpStream,
}

impl TanLogsClient {
    pub fn new(addr: impl ToSocketAddrs) -> Self {
        let tcp_stream = TcpStream::connect(addr).unwrap();
        println!("SDK Client connected to ingestor");
        Self { tcp_stream }
    }

    pub fn log_debug(&mut self, message: String) {
        let trace = Trace::debug(message);
        let bytes = trace.to_vec();
        self.tcp_stream.write_all(bytes.as_slice()).unwrap();
    }
}
