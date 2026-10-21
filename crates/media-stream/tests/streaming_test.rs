use reqwest::header::{ACCEPT_RANGES, AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use std::io::Write;
use tempfile::NamedTempFile;

use media_stream::MediaServer;

const TEST_FILE_SIZE: usize = 256 * 1024; // 256 KB
const VALID_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"; // 64 chars

fn create_test_file(size: usize) -> (NamedTempFile, Vec<u8>) {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let mut data = Vec::with_capacity(size);
    for i in 0..size {
        data.push(((i * 7 + 13) % 251) as u8);
    }
    file.write_all(&data).expect("Failed to write test data");
    file.flush().expect("Failed to flush file");
    (file, data)
}

fn sha256_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[tokio::test]
async fn test_full_content_streaming_success() {
    let (file, data) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(server.stream_url())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(CONTENT_LENGTH).unwrap(),
        &TEST_FILE_SIZE.to_string()
    );
    assert_eq!(resp.headers().get(ACCEPT_RANGES).unwrap(), "bytes");

    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), TEST_FILE_SIZE);
    assert_eq!(sha256_digest(&body), sha256_digest(&data));

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_bounded_byte_range_streaming_success() {
    let (file, data) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=0-1023")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes 0-1023/{}", TEST_FILE_SIZE)
    );
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1024");
    assert_eq!(resp.headers().get(ACCEPT_RANGES).unwrap(), "bytes");

    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 1024);
    assert_eq!(sha256_digest(&body), sha256_digest(&data[0..1024]));

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_open_ended_range_streaming_success() {
    let (file, data) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();

    let client = reqwest::Client::new();
    let start_offset = 100_000;
    let expected_len = TEST_FILE_SIZE - start_offset;

    let resp = client
        .get(server.stream_url())
        .header(RANGE, format!("bytes={}-", start_offset))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes {}-{}/{}", start_offset, TEST_FILE_SIZE - 1, TEST_FILE_SIZE)
    );
    assert_eq!(
        resp.headers().get(CONTENT_LENGTH).unwrap(),
        &expected_len.to_string()
    );

    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), expected_len);
    assert_eq!(sha256_digest(&body), sha256_digest(&data[start_offset..]));

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_suffix_range_streaming_success() {
    let (file, data) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();

    let client = reqwest::Client::new();
    let suffix_len = 4096;
    let expected_start = TEST_FILE_SIZE - suffix_len;

    let resp = client
        .get(server.stream_url())
        .header(RANGE, format!("bytes=-{}", suffix_len))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes {}-{}/{}", expected_start, TEST_FILE_SIZE - 1, TEST_FILE_SIZE)
    );
    assert_eq!(
        resp.headers().get(CONTENT_LENGTH).unwrap(),
        &suffix_len.to_string()
    );

    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), suffix_len);
    assert_eq!(sha256_digest(&body), sha256_digest(&data[expected_start..]));

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_unsatisfiable_range_returns_416() {
    let (file, _) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(server.stream_url())
        .header(RANGE, format!("bytes={}-", TEST_FILE_SIZE + 1000))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes */{}", TEST_FILE_SIZE)
    );

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_malformed_range_returns_400() {
    let (file, _) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=invalid-range")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_authorization_checks() {
    let (file, _) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();

    let client = reqwest::Client::new();

    // 1. Missing token -> 403
    let base_url = format!("http://{}/video/stream", server.local_addr);
    let resp = client.get(&base_url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 2. Wrong token in query -> 403
    let wrong_query_url = format!("http://{}/video/stream?token=invalid_token", server.local_addr);
    let resp = client.get(&wrong_query_url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 3. Valid token in Bearer header -> 200
    let resp = client
        .get(&base_url)
        .header(AUTHORIZATION, format!("Bearer {}", VALID_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_path_isolation_and_anti_traversal() {
    let (file, _) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();

    let client = reqwest::Client::new();

    // Traversal attempts
    let traversal_url = format!(
        "http://{}/video/stream/../../../../etc/passwd?token={}",
        server.local_addr, VALID_TOKEN
    );
    let resp = client.get(&traversal_url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let other_url = format!("http://{}/etc/passwd?token={}", server.local_addr, VALID_TOKEN);
    let resp = client.get(&other_url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let root_url = format!("http://{}/?token={}", server.local_addr, VALID_TOKEN);
    let resp = client.get(&root_url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_range_streaming() {
    let (file, data) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let stream_url = server.stream_url();

    let data_arc = std::sync::Arc::new(data);
    let mut handles = Vec::new();

    for i in 0..8 {
        let url = stream_url.clone();
        let data_ref = data_arc.clone();
        let handle = tokio::spawn(async move {
            let start = i * 16384;
            let end = start + 8191;
            let client = reqwest::Client::new();
            let resp = client
                .get(&url)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let body = resp.bytes().await.unwrap();
            assert_eq!(body.len(), 8192);
            assert_eq!(sha256_digest(&body), sha256_digest(&data_ref[start..=end]));
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_server_shutdown_lifecycle() {
    let (file, _) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let url = server.stream_url();

    // Verify online
    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Shutdown
    server.shutdown().await.unwrap();

    // Subsequent request should fail to connect or return error
    let res = client.get(&url).send().await;
    match res {
        Ok(r) => assert!(r.status() == StatusCode::FORBIDDEN || !r.status().is_success()),
        Err(e) => assert!(e.is_connect() || e.is_request()),
    }
}
