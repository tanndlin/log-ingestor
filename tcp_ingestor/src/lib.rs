use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};
use util::Trace;
use writer::WalWriter;

pub async fn run(wal_writer: Arc<Mutex<WalWriter>>) -> Result<()> {
    let tcp_port = std::env::var("TCP_PORT").unwrap_or("1337".to_string());
    let tcp_listener = TcpListener::bind(format!("0.0.0.0:{tcp_port}"))
        .await
        .with_context(|| format!("binding TCP ingestor to port {tcp_port}"))?;
    loop {
        match tcp_listener.accept().await {
            Ok((stream, addr)) => {
                tokio::spawn(handle_client(stream, addr, wal_writer.clone()));
            }
            // Transient (e.g. too many open files); keep accepting
            Err(e) => eprintln!("TCP accept failed: {e}"),
        }
    }
}

async fn handle_client(stream: TcpStream, addr: SocketAddr, wal_writer: Arc<Mutex<WalWriter>>) {
    println!("Conencted to peer: {addr:?}");

    let mut frames = FramedRead::new(stream, LengthDelimitedCodec::new());

    while let Some(frame) = frames.next().await {
        let frame = match frame {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Frame error from {addr}: {e}");
                break;
            }
        };
        match postcard::from_bytes::<Trace>(&frame) {
            Ok(trace) => {
                dbg!(&trace);
                // WalWriter::write has no panic paths, so a poisoned lock leaves it usable
                let mut wal_writer = wal_writer.lock().unwrap_or_else(PoisonError::into_inner);
                if let Err(e) = wal_writer.write(&trace) {
                    eprintln!("WAL write failed: {e:#}");
                }
            }
            Err(e) => {
                eprintln!("Failed to decode frame: {e}");
                break;
            }
        }
    }
}
