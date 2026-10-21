//! Chunked async file reader using Tokio and ReaderStream.

use axum::body::Body;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio_util::io::ReaderStream;

use crate::error::MediaStreamError;
use crate::range::ResolvedByteRange;

/// Standard streaming buffer chunk size (128 KB).
pub const STREAM_CHUNK_SIZE: usize = 128 * 1024;

/// Asynchronously opens the video file, seeks to `range.start`, limits reading to `range.content_length()`,
/// and returns an Axum streaming Body.
pub async fn create_byte_range_stream(
    file_path: &Path,
    range: ResolvedByteRange,
) -> Result<Body, MediaStreamError> {
    let mut file = File::open(file_path).await.map_err(MediaStreamError::Io)?;
    file.seek(SeekFrom::Start(range.start))
        .await
        .map_err(MediaStreamError::Io)?;

    let limited_reader = file.take(range.content_length());
    let stream = ReaderStream::with_capacity(limited_reader, STREAM_CHUNK_SIZE);
    Ok(Body::from_stream(stream))
}

/// Asynchronously streams the entire file from byte 0 to total_size.
pub async fn create_full_file_stream(file_path: &Path) -> Result<Body, MediaStreamError> {
    let file = File::open(file_path).await.map_err(MediaStreamError::Io)?;
    let stream = ReaderStream::with_capacity(file, STREAM_CHUNK_SIZE);
    Ok(Body::from_stream(stream))
}
