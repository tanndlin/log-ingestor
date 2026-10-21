use bytes::{Bytes, BytesMut};
use std::{
    io::Write,
    net::{TcpStream, ToSocketAddrs},
};
use tokio_util::codec::{Encoder, LengthDelimitedCodec};

use util::Trace;

pub struct TanLogsClient {
    tcp_stream: TcpStream,
    codec: LengthDelimitedCodec,
    buf: BytesMut,
}

impl TanLogsClient {
    pub fn new(addr: impl ToSocketAddrs) -> Self {
        let tcp_stream = TcpStream::connect(addr).unwrap();
        println!("SDK Client connected to ingestor");
        Self {
            tcp_stream,
            codec: LengthDelimitedCodec::new(),
            buf: BytesMut::new(),
        }
    }

    pub fn log_debug(&mut self, message: String) {
        let trace = Trace::debug(message);
        let bytes = trace.to_vec();

        self.buf.clear();
        self.codec
            .encode(Bytes::copy_from_slice(&bytes), &mut self.buf)
            .unwrap();
        self.tcp_stream.write_all(&self.buf).unwrap();
    }
}
