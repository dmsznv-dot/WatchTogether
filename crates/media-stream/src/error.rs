//! Structured error definitions for media streaming.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MediaStreamError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Target path is not a regular file: {0}")]
    NotARegularFile(PathBuf),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Malformed range specification: {0}")]
    MalformedRange(String),

    #[error("Range not satisfiable for file size {0}")]
    RangeNotSatisfiable(u64),

    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    #[error("Media server has shut down")]
    ServerShutdown,
}

impl IntoResponse for MediaStreamError {
    fn into_response(self) -> Response {
        match self {
            MediaStreamError::FileNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()).into_response(),
            MediaStreamError::NotARegularFile(_) => (StatusCode::BAD_REQUEST, self.to_string()).into_response(),
            MediaStreamError::Unauthorized(_) => (StatusCode::FORBIDDEN, self.to_string()).into_response(),
            MediaStreamError::MalformedRange(_) => (StatusCode::BAD_REQUEST, self.to_string()).into_response(),
            MediaStreamError::RangeNotSatisfiable(size) => {
                let mut resp = (StatusCode::RANGE_NOT_SATISFIABLE, self.to_string()).into_response();
                resp.headers_mut().insert(
                    axum::http::header::CONTENT_RANGE,
                    format!("bytes */{}", size).parse().unwrap(),
                );
                resp
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response(),
        }
    }
}
