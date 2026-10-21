//! Token authentication and constant-time token comparison.

use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use subtle::ConstantTimeEq;

/// Extracts the room token from query parameters (`?token=...`) or `Authorization: Bearer <token>`.
pub fn extract_token(query_params: &HashMap<String, String>, headers: &HeaderMap) -> Option<String> {
    // 1. Check query parameter `token`
    if let Some(token) = query_params.get("token") {
        if !token.is_empty() {
            return Some(token.clone());
        }
    }

    // 2. Check Authorization header
    if let Some(auth_val) = headers.get(axum::http::header::AUTHORIZATION) {
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

/// Validates the extracted token against the expected room token using constant-time comparison.
/// To eliminate length timing leaks, SHA-256 hashes of both strings are compared using `subtle::ConstantTimeEq`.
pub fn validate_token(provided_token: &str, expected_token: &str) -> bool {
    let provided_hash = Sha256::digest(provided_token.as_bytes());
    let expected_hash = Sha256::digest(expected_token.as_bytes());

    // Constant-time equality comparison between 32-byte arrays
    provided_hash.ct_eq(&expected_hash).into()
}
