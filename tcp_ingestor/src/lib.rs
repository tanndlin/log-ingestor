use futures_util::StreamExt;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};
use util::Trace;

pub async fn run() {
    // TODO: Pull port from env var
    let tcp_listener = TcpListener::bind("0.0.0.0:1337").await.unwrap();
    let local_addr = tcp_listener.local_addr().unwrap();
    println!("TCP Ingestor listen on: {local_addr}");
    loop {
        let (stream, addr) = tcp_listener.accept().await.unwrap();
        tokio::spawn(handle_client(stream, addr));
    }
}

async fn handle_client(stream: TcpStream, addr: SocketAddr) {
    println!("Conencted to peer: {addr:?}");

    let mut frames = FramedRead::new(stream, LengthDelimitedCodec::new());

    while let Some(Ok(frame)) = frames.next().await {
        match postcard::from_bytes::<Trace>(&frame) {
            Ok(trace) => {
                dbg!(trace);
            }
            Err(e) => {
                eprintln!("Failed to decode frame: {e}");
                break;
            }
        }
    }
}
