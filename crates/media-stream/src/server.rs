//! Embedded HTTP byte-range video streaming server.

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::auth::{extract_token, validate_token};
use crate::error::MediaStreamError;
use crate::range::{parse_range_header, resolve_range, RangeError};
use crate::stream::{create_byte_range_stream, create_full_file_stream};

#[derive(Clone)]
pub struct MediaServerState {
    pub canonical_video_path: Arc<PathBuf>,
    pub file_size: u64,
    pub mime_type: String,
    pub room_token: String,
    pub is_active: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct MediaServerConfig {
    pub bind_addr: SocketAddr,
    pub video_path: PathBuf,
    pub room_token: String,
}

pub struct MediaServerHandle {
    pub local_addr: SocketAddr,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub join_handle: tokio::task::JoinHandle<()>,
    pub state: MediaServerState,
}

impl MediaServerHandle {
    pub fn stream_url(&self) -> String {
        format!("http://{}/video/stream?token={}", self.local_addr, self.state.room_token)
    }

    pub async fn shutdown(mut self) -> Result<(), MediaStreamError> {
        self.state.is_active.store(false, Ordering::SeqCst);
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.join_handle.await.map_err(|_| MediaStreamError::ServerShutdown)?;
        Ok(())
    }
}

pub struct MediaServer;

impl MediaServer {
    pub async fn start(config: MediaServerConfig) -> Result<MediaServerHandle, MediaStreamError> {
        let canonical_path = std::fs::canonicalize(&config.video_path)
            .map_err(|_| MediaStreamError::FileNotFound(config.video_path.clone()))?;

        if !canonical_path.is_file() {
            return Err(MediaStreamError::NotARegularFile(canonical_path));
        }

        let metadata = tokio::fs::metadata(&canonical_path).await?;
        let file_size = metadata.len();

        let mime_type = match canonical_path.extension().and_then(|ext| ext.to_str()) {
            Some("mkv") => "video/x-matroska".to_string(),
            Some("mp4") => "video/mp4".to_string(),
            Some("mov") => "video/quicktime".to_string(),
            _ => mime_guess::from_path(&canonical_path)
                .first_or_octet_stream()
                .to_string(),
        };

        let state = MediaServerState {
            canonical_video_path: Arc::new(canonical_path),
            file_size,
            mime_type,
            room_token: config.room_token,
            is_active: Arc::new(AtomicBool::new(true)),
        };

        let app = Router::new()
            .route("/video/stream", get(handle_stream).head(handle_stream_head))
            .fallback(handle_fallback)
            .with_state(state.clone());

        let listener = TcpListener::bind(config.bind_addr).await?;
        let local_addr = listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let join_handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap_or_default();
        });

        Ok(MediaServerHandle {
            local_addr,
            shutdown_tx: Some(shutdown_tx),
            join_handle,
            state,
        })
    }

    pub async fn start_local(video_path: &Path, room_token: &str) -> Result<MediaServerHandle, MediaStreamError> {
        let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        Self::start(MediaServerConfig {
            bind_addr,
            video_path: video_path.to_path_buf(),
            room_token: room_token.to_string(),
        }).await
    }
}

async fn handle_stream(
    State(state): State<MediaServerState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if !state.is_active.load(Ordering::SeqCst) {
        return (StatusCode::FORBIDDEN, "Server closed").into_response();
    }

    // Token authentication
    let provided_token = match extract_token(&query, &headers) {
        Some(t) => t,
        None => return (StatusCode::FORBIDDEN, "Missing room token").into_response(),
    };

    if !validate_token(&provided_token, &state.room_token) {
        return (StatusCode::FORBIDDEN, "Invalid room token").into_response();
    }

    // Range handling
    if let Some(range_header) = headers.get(header::RANGE) {
        let range_str = match range_header.to_str() {
            Ok(s) => s,
            Err(_) => return (StatusCode::BAD_REQUEST, "Malformed Range header").into_response(),
        };

        let parsed = match parse_range_header(range_str) {
            Ok(p) => p,
            Err(RangeError::InvalidUnit) => {
                // Ignore unrecognized range unit, serve full file (RFC 9110 §14.2)
                return serve_full_file(&state).await;
            }
            Err(RangeError::MalformedHeader(_)) | Err(RangeError::MultipleRangesUnsupported) => {
                return (StatusCode::BAD_REQUEST, "Malformed or unsupported Range header").into_response();
            }
            Err(RangeError::Unsatisfiable { total_size: _ }) => {
                let mut resp = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
                resp.headers_mut().insert(
                    header::CONTENT_RANGE,
                    RangeError::unsatisfiable_header(state.file_size).parse().unwrap(),
                );
                resp.headers_mut().insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
                return resp;
            }
        };

        match resolve_range(parsed, state.file_size) {
            Ok(resolved) => {
                match create_byte_range_stream(&state.canonical_video_path, resolved).await {
                    Ok(body) => {
                        Response::builder()
                            .status(StatusCode::PARTIAL_CONTENT)
                            .header(header::CONTENT_TYPE, &state.mime_type)
                            .header(header::CONTENT_RANGE, resolved.content_range_header())
                            .header(header::CONTENT_LENGTH, resolved.content_length().to_string())
                            .header(header::ACCEPT_RANGES, "bytes")
                            .header(header::CACHE_CONTROL, "no-cache, private")
                            .header(header::CONNECTION, "keep-alive")
                            .body(body)
                            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
                    }
                    Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file").into_response(),
                }
            }
            Err(RangeError::Unsatisfiable { total_size }) => {
                let mut resp = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
                resp.headers_mut().insert(
                    header::CONTENT_RANGE,
                    RangeError::unsatisfiable_header(total_size).parse().unwrap(),
                );
                resp.headers_mut().insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
                resp
            }
            Err(_) => (StatusCode::BAD_REQUEST, "Range error").into_response(),
        }
    } else {
        serve_full_file(&state).await
    }
}

async fn serve_full_file(state: &MediaServerState) -> Response {
    match create_full_file_stream(&state.canonical_video_path).await {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, &state.mime_type)
            .header(header::CONTENT_LENGTH, state.file_size.to_string())
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, "no-cache, private")
            .body(body)
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file").into_response(),
    }
}

async fn handle_stream_head(
    State(state): State<MediaServerState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if !state.is_active.load(Ordering::SeqCst) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let provided_token = match extract_token(&query, &headers) {
        Some(t) => t,
        None => return StatusCode::FORBIDDEN.into_response(),
    };
    if !validate_token(&provided_token, &state.room_token) {
        return StatusCode::FORBIDDEN.into_response();
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &state.mime_type)
        .header(header::CONTENT_LENGTH, state.file_size.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-cache, private")
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn handle_fallback() -> Response {
    (StatusCode::NOT_FOUND, "Not Found").into_response()
}
