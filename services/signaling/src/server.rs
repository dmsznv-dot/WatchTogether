use crate::room::RoomRegistry;
use crate::session::handle_signaling_socket;
use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub struct SignalingServerHandle {
    pub local_addr: SocketAddr,
    pub registry: RoomRegistry,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub join_handle: tokio::task::JoinHandle<()>,
}

impl SignalingServerHandle {
    pub async fn shutdown(mut self) -> Result<(), std::io::Error> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.join_handle
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(())
    }
}

pub struct SignalingServer;

impl SignalingServer {
    pub fn create_router(registry: RoomRegistry) -> Router {
        Router::new()
            .route("/ws", get(ws_handler))
            .route("/health", get(health_handler))
            .with_state(registry)
    }

    pub async fn start(
        bind_addr: SocketAddr,
        registry: RoomRegistry,
    ) -> Result<SignalingServerHandle, std::io::Error> {
        let app = Self::create_router(registry.clone());
        let listener = TcpListener::bind(bind_addr).await?;
        let local_addr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let join_handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap_or_default();
        });

        Ok(SignalingServerHandle {
            local_addr,
            registry,
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        })
    }

    pub async fn start_default(bind_addr: SocketAddr) -> Result<SignalingServerHandle, std::io::Error> {
        Self::start(bind_addr, RoomRegistry::new()).await
    }
}

async fn health_handler(State(registry): State<RoomRegistry>) -> impl IntoResponse {
    let active_rooms = registry.active_rooms_count().await;
    let active_peers = registry.active_peers_count().await;
    Json(json!({
        "status": "ok",
        "active_rooms": active_rooms,
        "active_peers": active_peers,
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(registry): State<RoomRegistry>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket: WebSocket| handle_signaling_socket(socket, registry))
}
