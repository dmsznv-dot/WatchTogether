use crate::error::{NetworkError, NetworkResult};
use crate::p2p::{P2pTransport, TunnelRequest, TunnelResponse};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<TunnelResponse>>>>;

#[derive(Clone)]
struct ProxyState {
    transport: Arc<dyn P2pTransport>,
    next_req_id: Arc<AtomicU64>,
    pending: PendingMap,
}

#[derive(Debug, Clone)]
pub struct LoopbackProxyConfig {
    pub bind_addr: SocketAddr,
}

impl Default for LoopbackProxyConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
        }
    }
}

pub struct LoopbackProxyHandle {
    pub local_addr: SocketAddr,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub join_handle: tokio::task::JoinHandle<()>,
}

impl LoopbackProxyHandle {
    pub fn stream_url(&self, token: &str) -> String {
        format!("http://{}/video/stream?token={}", self.local_addr, token)
    }

    pub async fn shutdown(mut self) -> NetworkResult<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.join_handle
            .await
            .map_err(|e| NetworkError::Proxy(e.to_string()))?;
        Ok(())
    }
}

pub struct LoopbackProxy;

impl LoopbackProxy {
    pub async fn start(
        transport: Arc<dyn P2pTransport>,
        config: LoopbackProxyConfig,
    ) -> NetworkResult<LoopbackProxyHandle> {
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let state = ProxyState {
            transport: transport.clone(),
            next_req_id: Arc::new(AtomicU64::new(1)),
            pending: pending.clone(),
        };

        let transport_rx = transport.clone();
        let pending_rx = pending.clone();
        let receiver_handle = tokio::spawn(async move {
            while let Ok(resp) = transport_rx.recv_media_response().await {
                if let Some(tx) = pending_rx.lock().await.remove(&resp.request_id) {
                    let _ = tx.send(resp);
                }
            }
            pending_rx.lock().await.clear();
        });

        let app = Router::new()
            .route("/video/stream", get(handle_proxy_stream))
            .fallback(handle_proxy_fallback)
            .with_state(state);

        let listener = TcpListener::bind(config.bind_addr).await?;
        let local_addr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let receiver_abort = receiver_handle.abort_handle();
        let pending_shutdown = pending.clone();
        let join_handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                    receiver_abort.abort();
                    pending_shutdown.lock().await.clear();
                })
                .await;
        });

        Ok(LoopbackProxyHandle {
            local_addr,
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        })
    }
}

async fn handle_proxy_stream(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let req_id = state.next_req_id.fetch_add(1, Ordering::SeqCst);
    let mut forwarded_headers = Vec::new();
    for (name, val) in headers.iter() {
        if let Ok(v_str) = val.to_str() {
            forwarded_headers.push((name.to_string(), v_str.to_string()));
        }
    }

    let (tx, rx) = oneshot::channel();
    {
        let mut map = state.pending.lock().await;
        map.insert(req_id, tx);
    }

    let tunnel_req = TunnelRequest {
        request_id: req_id,
        method: "GET".into(),
        uri: uri.to_string(),
        headers: forwarded_headers,
    };

    if let Err(e) = state.transport.send_media_request(tunnel_req).await {
        state.pending.lock().await.remove(&req_id);
        return (
            StatusCode::BAD_GATEWAY,
            format!("P2P tunnel send failed: {}", e),
        )
            .into_response();
    }

    let timeout_duration = std::time::Duration::from_secs(30);
    match tokio::time::timeout(timeout_duration, rx).await {
        Ok(Ok(tunnel_resp)) => {
            let mut resp_builder = Response::builder().status(
                StatusCode::from_u16(tunnel_resp.status)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            );
            for (h_name, h_val) in tunnel_resp.headers {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(h_name.as_bytes()),
                    HeaderValue::from_str(&h_val),
                ) {
                    resp_builder = resp_builder.header(name, val);
                }
            }
            resp_builder
                .body(Body::from(tunnel_resp.body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Ok(Err(_)) => {
            state.pending.lock().await.remove(&req_id);
            (
                StatusCode::BAD_GATEWAY,
                "P2P tunnel connection closed before response",
            )
                .into_response()
        }
        Err(_) => {
            state.pending.lock().await.remove(&req_id);
            (
                StatusCode::GATEWAY_TIMEOUT,
                "P2P tunnel request timed out",
            )
                .into_response()
        }
    }
}

async fn handle_proxy_fallback() -> Response {
    (StatusCode::NOT_FOUND, "Not Found").into_response()
}
