//! RFC 7233 / RFC 9110 Range header parser and range resolver.

/// Represents a parsed byte range specification prior to total size resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedRange {
    /// bytes=start-end
    Bounded { start: u64, end: u64 },
    /// bytes=start-
    Open { start: u64 },
    /// bytes=-suffix
    Suffix { suffix: u64 },
}

/// A resolved byte range clamped to total file size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedByteRange {
    pub start: u64,
    pub end: u64,
    pub total_size: u64,
}

impl ResolvedByteRange {
    /// Number of bytes in this range: (end - start + 1)
    pub fn content_length(&self) -> u64 {
        self.end - self.start + 1
    }

    /// Generates the Content-Range header value for HTTP 206 responses.
    pub fn content_range_header(&self) -> String {
        format!("bytes {}-{}/{}", self.start, self.end, self.total_size)
    }
}

/// Errors occurring during range header parsing or resolution.
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
    /// Formats the Content-Range header required for HTTP 416 responses.
    pub fn unsatisfiable_header(total_size: u64) -> String {
        format!("bytes */{}", total_size)
    }
}

/// Parses the raw `Range` HTTP header string according to RFC 9110 §14.1.2.
pub fn parse_range_header(header_val: &str) -> Result<ParsedRange, RangeError> {
    let header_val = header_val.trim();
    if !header_val.starts_with("bytes=") {
        return Err(RangeError::InvalidUnit);
    }

    let spec_part = &header_val["bytes=".len()..].trim();
    if spec_part.is_empty() {
        return Err(RangeError::MalformedHeader("Empty byte range spec".into()));
    }

    // Disallow multiple comma-separated ranges for single-stream video playback
    if spec_part.contains(',') {
        return Err(RangeError::MultipleRangesUnsupported);
    }

    if let Some(suffix_str) = spec_part.strip_prefix('-') {
        // Suffix range: bytes=-suffix
        let suffix = suffix_str
            .parse::<u64>()
            .map_err(|_| RangeError::MalformedHeader("Invalid suffix value".into()))?;
        if suffix == 0 {
            // Suffix length 0 is unsatisfiable / malformed
            return Err(RangeError::MalformedHeader("Suffix length cannot be 0".into()));
        }
        Ok(ParsedRange::Suffix { suffix })
    } else if let Some(start_str) = spec_part.strip_suffix('-') {
        // Open range: bytes=start-
        let start = start_str
            .parse::<u64>()
            .map_err(|_| RangeError::MalformedHeader("Invalid start value".into()))?;
        Ok(ParsedRange::Open { start })
    } else if let Some((start_str, end_str)) = spec_part.split_once('-') {
        // Bounded range: bytes=start-end
        let start = start_str
            .parse::<u64>()
            .map_err(|_| RangeError::MalformedHeader("Invalid bounded start".into()))?;
        let end = end_str
            .parse::<u64>()
            .map_err(|_| RangeError::MalformedHeader("Invalid bounded end".into()))?;
        if start > end {
            // RFC 9110: start > end is syntactically unsatisfiable
            return Err(RangeError::Unsatisfiable { total_size: 0 });
        }
        Ok(ParsedRange::Bounded { start, end })
    } else {
        Err(RangeError::MalformedHeader("Missing '-' separator".into()))
    }
}

/// Resolves a `ParsedRange` against the actual file size.
pub fn resolve_range(parsed: ParsedRange, total_size: u64) -> Result<ResolvedByteRange, RangeError> {
    if total_size == 0 {
        return Err(RangeError::Unsatisfiable { total_size });
    }

    match parsed {
        ParsedRange::Bounded { start, end } => {
            if start >= total_size || start > end {
                return Err(RangeError::Unsatisfiable { total_size });
            }
            // RFC 9110 §14.1.2: If last-pos >= total_size, clamp to total_size - 1
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
            // RFC 9110 §14.1.2: If suffix >= total_size, serve whole file
            let start = total_size.saturating_sub(suffix);
            Ok(ResolvedByteRange {
                start,
                end: total_size - 1,
                total_size,
            })
        }
    }
}
