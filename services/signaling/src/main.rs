use signaling::{RoomRegistry, SignalingServer};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()
        .unwrap_or(8080);
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    tracing::info!("Starting signaling service on {}", addr);
    let handle = SignalingServer::start(addr, RoomRegistry::new()).await?;

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down signaling service...");
    handle.shutdown().await?;
    Ok(())
}
