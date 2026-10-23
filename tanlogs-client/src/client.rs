use anyhow::{Context, Result, anyhow, bail};
use bytes::{Bytes, BytesMut};
use std::{
    io::{self, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
use tokio_util::codec::{Encoder, LengthDelimitedCodec};

use util::{Level, Trace};

/// Sends block the caller, so a dead ingestor must not stall them for long.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const INITIAL_BACKOFF: Duration = Duration::from_millis(100);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

pub struct TanLogsClient {
    addrs: Vec<SocketAddr>,
    /// `None` after a failed write until a reconnect succeeds.
    tcp_stream: Option<TcpStream>,
    next_reconnect: Instant,
    backoff: Duration,
    codec: LengthDelimitedCodec,
    buf: BytesMut,
    application_name: String,
}

impl TanLogsClient {
    pub fn connect(addr: impl ToSocketAddrs, application_name: impl Into<String>) -> Result<Self> {
        let addrs: Vec<_> = addr.to_socket_addrs()?.collect();
        let tcp_stream = open_stream(&addrs)?;
        Ok(Self {
            addrs,
            tcp_stream: Some(tcp_stream),
            next_reconnect: Instant::now(),
            backoff: INITIAL_BACKOFF,
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
        self.write_frame().context("sending trace to ingestor")
    }

    /// Writes `buf`, reconnecting once if the connection has dropped. While the
    /// ingestor stays unreachable, traces are dropped and reconnects back off.
    fn write_frame(&mut self) -> Result<()> {
        if let Some(stream) = &mut self.tcp_stream {
            match stream.write_all(&self.buf) {
                Ok(()) => return Ok(()),
                // The ingestor likely restarted, so try a fresh connection right away.
                // A partial frame dies with the old stream, so resending in full is safe.
                Err(e) => {
                    self.tcp_stream = None;
                    self.next_reconnect = Instant::now();
                    eprintln!("tanlogs: connection lost, reconnecting: {e}");
                }
            }
        }

        if Instant::now() < self.next_reconnect {
            bail!("not connected, dropping trace");
        }

        let mut stream = match open_stream(&self.addrs) {
            Ok(stream) => stream,
            Err(e) => {
                self.next_reconnect = Instant::now() + self.backoff;
                self.backoff = (self.backoff * 2).min(MAX_BACKOFF);
                return Err(e.context("reconnecting"));
            }
        };
        self.backoff = INITIAL_BACKOFF;
        stream.write_all(&self.buf)?;
        self.tcp_stream = Some(stream);
        Ok(())
    }
}

fn open_stream(addrs: &[SocketAddr]) -> Result<TcpStream> {
    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(addr, CONNECT_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(e) => last_err = Some(e),
        }
    }
    Err(anyhow!(last_err.unwrap_or_else(|| io::Error::other(
        "no addresses to connect to"
    ))))
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
