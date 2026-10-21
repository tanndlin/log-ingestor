use bytes::{Bytes, BytesMut};
use std::{
    io::Write,
    net::{TcpStream, ToSocketAddrs},
};
use tokio_util::codec::{Encoder, LengthDelimitedCodec};

use util::{Level, Trace};

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

    pub fn fatal(&mut self, message: String) {
        self.send_trace(&Trace::new(Level::Fatal, message));
    }

    pub fn error(&mut self, message: String) {
        self.send_trace(&Trace::new(Level::Error, message));
    }

    pub fn warn(&mut self, message: String) {
        self.send_trace(&Trace::new(Level::Warn, message));
    }

    pub fn info(&mut self, message: String) {
        self.send_trace(&Trace::new(Level::Info, message));
    }

    pub fn debug(&mut self, message: String) {
        self.send_trace(&Trace::new(Level::Debug, message));
    }

    pub fn trace(&mut self, message: String) {
        self.send_trace(&Trace::new(Level::Trace, message));
    }

    fn send_trace(&mut self, trace: &Trace) {
        let bytes = trace.to_vec();

        self.buf.clear();
        self.codec
            .encode(Bytes::copy_from_slice(&bytes), &mut self.buf)
            .unwrap();
        self.tcp_stream.write_all(&self.buf).unwrap();
    }
}
