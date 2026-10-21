//! Embedded HTTP byte-range video streaming server for Watch Together.

pub mod auth;
pub mod error;
pub mod range;
pub mod server;
pub mod stream;

pub use auth::{extract_token, validate_token};
pub use error::MediaStreamError;
pub use range::{parse_range_header, resolve_range, ParsedRange, RangeError, ResolvedByteRange};
pub use server::{MediaServer, MediaServerConfig, MediaServerHandle, MediaServerState};
pub use stream::{create_byte_range_stream, create_full_file_stream, STREAM_CHUNK_SIZE};
