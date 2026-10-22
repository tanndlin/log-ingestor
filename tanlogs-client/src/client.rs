use anyhow::{Context, Result};
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
    application_name: String,
}

impl TanLogsClient {
    pub fn connect(addr: impl ToSocketAddrs, application_name: impl Into<String>) -> Result<Self> {
        let tcp_stream = TcpStream::connect(addr)?;
        Ok(Self {
            tcp_stream,
            codec: LengthDelimitedCodec::new(),
            buf: BytesMut::new(),
            application_name: application_name.into(),
        })
    }

    /// Returns a handle that tags every trace it sends with `span_id`.
    pub fn span(&mut self, span_id: i64) -> SpannedClient<'_> {
        SpannedClient {
            client: self,
            span_id,
        }
    }

    // TODO create macro off of the `Level` enum maybe
    pub fn fatal(&mut self, message: String) -> Result<()> {
        self.send(Level::Fatal, message, None)
    }

    pub fn error(&mut self, message: String) -> Result<()> {
        self.send(Level::Error, message, None)
    }

    pub fn warn(&mut self, message: String) -> Result<()> {
        self.send(Level::Warn, message, None)
    }

    pub fn info(&mut self, message: String) -> Result<()> {
        self.send(Level::Info, message, None)
    }

    pub fn debug(&mut self, message: String) -> Result<()> {
        self.send(Level::Debug, message, None)
    }

    pub fn trace(&mut self, message: String) -> Result<()> {
        self.send(Level::Trace, message, None)
    }

    pub fn send(&mut self, level: Level, message: String, span_id: Option<i64>) -> Result<()> {
        let trace = Trace::new(level, message, self.application_name.clone(), span_id);
        let bytes = trace.to_vec().context("serializing trace")?;

        self.buf.clear();
        self.codec
            .encode(Bytes::copy_from_slice(&bytes), &mut self.buf)
            .context("encoding trace frame")?;
        self.tcp_stream
            .write_all(&self.buf)
            .context("sending trace to ingestor")?;
        Ok(())
    }
}

pub struct SpannedClient<'a> {
    client: &'a mut TanLogsClient,
    span_id: i64,
}

impl SpannedClient<'_> {
    pub fn fatal(&mut self, message: String) -> Result<()> {
        self.send(Level::Fatal, message)
    }

    pub fn error(&mut self, message: String) -> Result<()> {
        self.send(Level::Error, message)
    }

    pub fn warn(&mut self, message: String) -> Result<()> {
        self.send(Level::Warn, message)
    }

    pub fn info(&mut self, message: String) -> Result<()> {
        self.send(Level::Info, message)
    }

    pub fn debug(&mut self, message: String) -> Result<()> {
        self.send(Level::Debug, message)
    }

    pub fn trace(&mut self, message: String) -> Result<()> {
        self.send(Level::Trace, message)
    }

    pub fn send(&mut self, level: Level, message: String) -> Result<()> {
        self.client.send(level, message, Some(self.span_id))
    }
}
