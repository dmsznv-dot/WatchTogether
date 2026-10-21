use axum::http::{header, HeaderMap, HeaderValue};
use media_stream::{extract_token, validate_token};
use std::collections::HashMap;

#[test]
fn test_auth_valid_query_param() {
    let mut query = HashMap::new();
    query.insert("token".to_string(), "0123456789abcdef0123456789abcdef".to_string());
    let headers = HeaderMap::new();

    let extracted = extract_token(&query, &headers);
    assert_eq!(extracted.as_deref(), Some("0123456789abcdef0123456789abcdef"));
    assert!(validate_token(&extracted.unwrap(), "0123456789abcdef0123456789abcdef"));
}

#[test]
fn test_auth_valid_bearer_header() {
    let query = HashMap::new();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer 0123456789abcdef0123456789abcdef"),
    );

    let extracted = extract_token(&query, &headers);
    assert_eq!(extracted.as_deref(), Some("0123456789abcdef0123456789abcdef"));
    assert!(validate_token(&extracted.unwrap(), "0123456789abcdef0123456789abcdef"));
}

#[test]
fn test_auth_missing_token() {
    let query = HashMap::new();
    let headers = HeaderMap::new();
    assert_eq!(extract_token(&query, &headers), None);
}

#[test]
fn test_auth_invalid_token() {
    let expected = "0123456789abcdef0123456789abcdef";
    let wrong = "fedcba9876543210fedcba9876543210";
    assert!(!validate_token(wrong, expected));
}

#[test]
fn test_auth_empty_token() {
    let mut query = HashMap::new();
    query.insert("token".to_string(), "".to_string());
    let headers = HeaderMap::new();
    assert_eq!(extract_token(&query, &headers), None);
}

#[test]
fn test_auth_length_mismatch() {
    let expected = "0123456789abcdef0123456789abcdef";
    let shorter = "0123456789abcdef";
    assert!(!validate_token(shorter, expected));
}
