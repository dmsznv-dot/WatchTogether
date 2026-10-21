use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_util::io::ReaderStream;

/// 128 KB buffer chunk size for streaming video chunks.
pub const STREAM_CHUNK_SIZE: usize = 128 * 1024;

/// Parsed byte range representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedRange {
    Bounded { start: u64, end: u64 },
    Open { start: u64 },
    Suffix { suffix: u64 },
}

/// Resolved byte range bounded by file size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedByteRange {
    pub start: u64,
    pub end: u64,
    pub total_size: u64,
}

impl ResolvedByteRange {
    pub fn content_length(&self) -> u64 {
        self.end - self.start + 1
    }

    pub fn content_range_header(&self) -> String {
        format!("bytes {}-{}/{}", self.start, self.end, self.total_size)
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RangeError {
    #[error("Invalid range unit (expected 'bytes')")]
    InvalidUnit,
    #[error("Malformed range header: {0}")]
    MalformedHeader(String),
    #[error("Range not satisfiable for file size {total_size}")]
    Unsatisfiable { total_size: u64 },
    #[error("Multipart/multiple ranges not supported")]
    MultipleRangesUnsupported,
}

impl RangeError {
    pub fn unsatisfiable_header(total_size: u64) -> String {
        format!("bytes */{}", total_size)
    }
}

pub fn parse_range_header(header_val: &str) -> Result<ParsedRange, RangeError> {
    let header_val = header_val.trim();
    if !header_val.starts_with("bytes=") {
        return Err(RangeError::InvalidUnit);
    }

    let spec_part = &header_val["bytes=".len()..].trim();
    if spec_part.is_empty() {
        return Err(RangeError::MalformedHeader("Empty byte range spec".into()));
    }

    if spec_part.contains(',') {
        return Err(RangeError::MultipleRangesUnsupported);
    }

    if let Some(suffix_str) = spec_part.strip_prefix('-') {
        let suffix = suffix_str
            .parse::<u64>()
            .map_err(|_| RangeError::MalformedHeader("Invalid suffix value".into()))?;
        if suffix == 0 {
            return Err(RangeError::MalformedHeader("Suffix length cannot be 0".into()));
        }
        Ok(ParsedRange::Suffix { suffix })
    } else if let Some(start_str) = spec_part.strip_suffix('-') {
        let start = start_str
            .parse::<u64>()
            .map_err(|_| RangeError::MalformedHeader("Invalid start value".into()))?;
        Ok(ParsedRange::Open { start })
    } else if let Some((start_str, end_str)) = spec_part.split_once('-') {
        let start = start_str
            .parse::<u64>()
            .map_err(|_| RangeError::MalformedHeader("Invalid bounded start".into()))?;
        let end = end_str
            .parse::<u64>()
            .map_err(|_| RangeError::MalformedHeader("Invalid bounded end".into()))?;
        if start > end {
            return Err(RangeError::Unsatisfiable { total_size: 0 });
        }
        Ok(ParsedRange::Bounded { start, end })
    } else {
        Err(RangeError::MalformedHeader("Missing '-' separator".into()))
    }
}

pub fn resolve_range(parsed: ParsedRange, total_size: u64) -> Result<ResolvedByteRange, RangeError> {
    if total_size == 0 {
        return Err(RangeError::Unsatisfiable { total_size });
    }

    match parsed {
        ParsedRange::Bounded { start, end } => {
            if start >= total_size || start > end {
                return Err(RangeError::Unsatisfiable { total_size });
            }
            let clamped_end = end.min(total_size - 1);
            Ok(ResolvedByteRange {
                start,
                end: clamped_end,
                total_size,
            })
        }
        ParsedRange::Open { start } => {
            if start >= total_size {
                return Err(RangeError::Unsatisfiable { total_size });
            }
            Ok(ResolvedByteRange {
                start,
                end: total_size - 1,
                total_size,
            })
        }
        ParsedRange::Suffix { suffix } => {
            if suffix == 0 {
                return Err(RangeError::Unsatisfiable { total_size });
            }
            let start = total_size.saturating_sub(suffix);
            Ok(ResolvedByteRange {
                start,
                end: total_size - 1,
                total_size,
            })
        }
    }
}

pub fn extract_token(query_params: &HashMap<String, String>, headers: &HeaderMap) -> Option<String> {
    if let Some(token) = query_params.get("token") {
        if !token.is_empty() {
            return Some(token.clone());
        }
    }

    if let Some(auth_val) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_val.to_str() {
            let auth_str = auth_str.trim();
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let token = token.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }

    None
}

pub fn validate_token(provided_token: &str, expected_token: &str) -> bool {
    let provided_hash = Sha256::digest(provided_token.as_bytes());
    let expected_hash = Sha256::digest(expected_token.as_bytes());
    provided_hash.ct_eq(&expected_hash).into()
}

#[derive(Clone)]
pub struct TestServerState {
    pub canonical_video_path: Arc<PathBuf>,
    pub file_size: u64,
    pub mime_type: String,
    pub room_token: String,
    pub is_active: Arc<AtomicBool>,
}

pub struct TestServerHandle {
    pub local_addr: SocketAddr,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub join_handle: tokio::task::JoinHandle<()>,
    pub state: TestServerState,
}

impl TestServerHandle {
    pub fn stream_url(&self) -> String {
        format!("http://{}/video/stream?token={}", self.local_addr, self.state.room_token)
    }

    pub async fn shutdown(mut self) {
        self.state.is_active.store(false, Ordering::SeqCst);
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.join_handle.await;
    }
}

pub struct TestMediaServer;

impl TestMediaServer {
    pub async fn start(video_path: &Path, room_token: &str) -> Result<TestServerHandle, Box<dyn std::error::Error + Send + Sync>> {
        let canonical_path = std::fs::canonicalize(video_path)?;
        let metadata = tokio::fs::metadata(&canonical_path).await?;
        let file_size = metadata.len();

        let mime_type = match canonical_path.extension().and_then(|ext| ext.to_str()) {
            Some("mkv") => "video/x-matroska".to_string(),
            Some("mp4") => "video/mp4".to_string(),
            Some("mov") => "video/quicktime".to_string(),
            _ => "application/octet-stream".to_string(),
        };

        let state = TestServerState {
            canonical_video_path: Arc::new(canonical_path),
            file_size,
            mime_type,
            room_token: room_token.to_string(),
            is_active: Arc::new(AtomicBool::new(true)),
        };

        let app = Router::new()
            .route("/video/stream", get(handle_stream).head(handle_stream_head))
            .fallback(handle_fallback)
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await?;
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

        Ok(TestServerHandle {
            local_addr,
            shutdown_tx: Some(shutdown_tx),
            join_handle,
            state,
        })
    }
}

async fn handle_stream(
    State(state): State<TestServerState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if !state.is_active.load(Ordering::SeqCst) {
        return (StatusCode::FORBIDDEN, "Server closed").into_response();
    }

    let provided_token = match extract_token(&query, &headers) {
        Some(t) => t,
        None => return (StatusCode::FORBIDDEN, "Missing room token").into_response(),
    };

    if !validate_token(&provided_token, &state.room_token) {
        return (StatusCode::FORBIDDEN, "Invalid room token").into_response();
    }

    if let Some(range_header) = headers.get(header::RANGE) {
        let range_str = match range_header.to_str() {
            Ok(s) => s,
            Err(_) => return (StatusCode::BAD_REQUEST, "Malformed Range header").into_response(),
        };

        let parsed = match parse_range_header(range_str) {
            Ok(p) => p,
            Err(RangeError::InvalidUnit) => {
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

async fn serve_full_file(state: &TestServerState) -> Response {
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
    State(state): State<TestServerState>,
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
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn handle_fallback() -> Response {
    (StatusCode::NOT_FOUND, "Not Found").into_response()
}

pub async fn create_byte_range_stream(
    file_path: &Path,
    range: ResolvedByteRange,
) -> Result<Body, std::io::Error> {
    let mut file = File::open(file_path).await?;
    file.seek(SeekFrom::Start(range.start)).await?;
    let limited_reader = file.take(range.content_length());
    let stream = ReaderStream::with_capacity(limited_reader, STREAM_CHUNK_SIZE);
    Ok(Body::from_stream(stream))
}

pub async fn create_full_file_stream(file_path: &Path) -> Result<Body, std::io::Error> {
    let file = File::open(file_path).await?;
    let stream = ReaderStream::with_capacity(file, STREAM_CHUNK_SIZE);
    Ok(Body::from_stream(stream))
}
