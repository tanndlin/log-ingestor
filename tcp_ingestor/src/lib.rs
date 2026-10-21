use futures_util::StreamExt;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};
use util::Trace;
use writer::WalWriter;

pub async fn run(wal_writer: Arc<Mutex<WalWriter>>) {
    // TODO: Pull port from env var
    let tcp_listener = TcpListener::bind("0.0.0.0:1337").await.unwrap();
    let local_addr = tcp_listener.local_addr().unwrap();
    println!("TCP Ingestor listen on: {local_addr}");
    loop {
        let (stream, addr) = tcp_listener.accept().await.unwrap();
        tokio::spawn(handle_client(stream, addr, wal_writer.clone()));
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
                wal_writer.lock().unwrap().write(&trace);
            }
            Err(e) => {
                eprintln!("Failed to decode frame: {e}");
                break;
            }
        }
    }
}
