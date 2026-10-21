use reqwest::header::{ACCEPT_RANGES, AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use reqwest::StatusCode;
use std::sync::Arc;
use tempfile::NamedTempFile;
use watch_together_tests::mock_media::{compute_range_sha256, compute_sha256, create_synthetic_media_file};
use watch_together_tests::test_server::TestMediaServer;

const TEST_FILE_SIZE: usize = 256 * 1024; // 256 KB test file
const VALID_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"; // 64 chars

// ============================================================================
// TIER 1: FEATURE COVERAGE (HAPPY PATHS)
// ============================================================================

#[tokio::test]
async fn test_tier1_streaming_bounded_range_success() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = create_synthetic_media_file(temp_file.path(), TEST_FILE_SIZE)
        .expect("Failed to write synthetic video");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN)
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=0-1023")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes 0-1023/{}", TEST_FILE_SIZE)
    );
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1024");
    assert_eq!(resp.headers().get(ACCEPT_RANGES).unwrap(), "bytes");

    let body_bytes = resp.bytes().await.expect("Failed to read body");
    assert_eq!(body_bytes.len(), 1024);
    assert_eq!(
        compute_sha256(&body_bytes),
        compute_range_sha256(&file_bytes, 0, 1023)
    );

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_streaming_open_ended_range_success() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = create_synthetic_media_file(temp_file.path(), TEST_FILE_SIZE)
        .expect("Failed to write synthetic video");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN)
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let start_offset = 100_000;
    let expected_len = TEST_FILE_SIZE - start_offset;

    let resp = client
        .get(server.stream_url())
        .header(RANGE, format!("bytes={}-", start_offset))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes {}-{}/{}", start_offset, TEST_FILE_SIZE - 1, TEST_FILE_SIZE)
    );
    assert_eq!(
        resp.headers().get(CONTENT_LENGTH).unwrap(),
        &expected_len.to_string()
    );

    let body_bytes = resp.bytes().await.expect("Failed to read body");
    assert_eq!(body_bytes.len(), expected_len);
    assert_eq!(
        compute_sha256(&body_bytes),
        compute_range_sha256(&file_bytes, start_offset, TEST_FILE_SIZE - 1)
    );

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_streaming_suffix_range_success() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = create_synthetic_media_file(temp_file.path(), TEST_FILE_SIZE)
        .expect("Failed to write synthetic video");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN)
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let suffix_len = 4096;
    let expected_start = TEST_FILE_SIZE - suffix_len;

    let resp = client
        .get(server.stream_url())
        .header(RANGE, format!("bytes=-{}", suffix_len))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes {}-{}/{}", expected_start, TEST_FILE_SIZE - 1, TEST_FILE_SIZE)
    );
    assert_eq!(
        resp.headers().get(CONTENT_LENGTH).unwrap(),
        &suffix_len.to_string()
    );

    let body_bytes = resp.bytes().await.expect("Failed to read body");
    assert_eq!(body_bytes.len(), suffix_len);
    assert_eq!(
        compute_sha256(&body_bytes),
        compute_range_sha256(&file_bytes, expected_start, TEST_FILE_SIZE - 1)
    );

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_streaming_full_file_no_range_header() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = create_synthetic_media_file(temp_file.path(), TEST_FILE_SIZE)
        .expect("Failed to write synthetic video");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN)
        .await
        .expect("Failed to start test server");

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

    let body_bytes = resp.bytes().await.expect("Failed to read body");
    assert_eq!(body_bytes.len(), TEST_FILE_SIZE);
    assert_eq!(compute_sha256(&body_bytes), compute_sha256(&file_bytes));

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_streaming_mid_file_slice_sha256() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = create_synthetic_media_file(temp_file.path(), TEST_FILE_SIZE)
        .expect("Failed to write synthetic video");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN)
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let (start, end) = (65536, 131071); // Exactly 64 KB in middle
    let resp = client
        .get(server.stream_url())
        .header(RANGE, format!("bytes={}-{}", start, end))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "65536");

    let body_bytes = resp.bytes().await.expect("Failed to read body");
    assert_eq!(
        compute_sha256(&body_bytes),
        compute_range_sha256(&file_bytes, start, end)
    );

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_auth_valid_token_query_param() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1024).expect("Failed to write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN)
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let url = format!("http://{}/video/stream?token={}", server.local_addr, VALID_TOKEN);
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=0-100")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_auth_valid_token_bearer_header() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1024).expect("Failed to write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN)
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();
    let url = format!("http://{}/video/stream", server.local_addr);
    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", VALID_TOKEN))
        .header(RANGE, "bytes=0-100")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    server.shutdown().await;
}

// ============================================================================
// TIER 2: BOUNDARY VALUE ANALYSIS & CORNER CASES
// ============================================================================

#[tokio::test]
async fn test_tier2_range_boundary_single_byte_start() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = create_synthetic_media_file(temp_file.path(), 1024).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1");
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes 0-0/1024");
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.len(), 1);
    assert_eq!(bytes[0], file_bytes[0]);

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_boundary_single_byte_end() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = create_synthetic_media_file(temp_file.path(), 1024).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=1023-1023")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1");
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes 1023-1023/1024");
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.len(), 1);
    assert_eq!(bytes[0], file_bytes[1023]);

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_boundary_clamped_at_eof() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    // RFC 9110 §14.1.2: If last-byte-pos >= file_size, last-byte-pos is clamped to file_size - 1
    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=900-5000")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "100");
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes 900-999/1000");

    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.len(), 100);
    assert_eq!(compute_sha256(&bytes), compute_range_sha256(&file_bytes, 900, 999));

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_unsatisfiable_start_greater_than_end() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=500-200")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes */1000");

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_unsatisfiable_start_at_or_beyond_eof() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    // Start == total_size
    let resp1 = client
        .get(server.stream_url())
        .header(RANGE, "bytes=1000-")
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(resp1.headers().get(CONTENT_RANGE).unwrap(), "bytes */1000");

    // Start > total_size
    let resp2 = client
        .get(server.stream_url())
        .header(RANGE, "bytes=2000-3000")
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(resp2.headers().get(CONTENT_RANGE).unwrap(), "bytes */1000");

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_unsatisfiable_zero_suffix() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=-0")
        .send()
        .await
        .unwrap();

    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::RANGE_NOT_SATISFIABLE);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_malformed_missing_hyphen() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=500")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_malformed_non_numeric() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=abc-def")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_multipart_rejected() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=0-10, 20-30")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_range_unrecognized_unit_fallback_to_full_file() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    // RFC 9110 §14.2: If unit is not 'bytes', range header must be ignored and full content served
    let resp = client
        .get(server.stream_url())
        .header(RANGE, "items=0-100")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1000");

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_auth_missing_token_rejected() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let url_without_token = format!("http://{}/video/stream", server.local_addr);
    let resp = client
        .get(&url_without_token)
        .header(RANGE, "bytes=0-100")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_auth_wrong_token_rejected() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let wrong_token = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let url_wrong_token = format!("http://{}/video/stream?token={}", server.local_addr, wrong_token);
    let resp = client
        .get(&url_wrong_token)
        .header(RANGE, "bytes=0-100")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_auth_empty_token_rejected() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let url_empty_token = format!("http://{}/video/stream?token=", server.local_addr);
    let resp = client
        .get(&url_empty_token)
        .header(RANGE, "bytes=0-100")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_security_path_traversal_rejection() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    // 1. Path traversal dot-dot attempt
    let traversal_url = format!(
        "http://{}/video/stream/../../../../etc/passwd?token={}",
        server.local_addr, VALID_TOKEN
    );
    let resp1 = client.get(&traversal_url).send().await.unwrap();
    assert!(resp1.status() == StatusCode::NOT_FOUND || resp1.status() == StatusCode::FORBIDDEN);

    // 2. Encoded traversal attempt (%2e%2e)
    let encoded_url = format!(
        "http://{}/video/stream/%2e%2e/%2e%2e/secret?token={}",
        server.local_addr, VALID_TOKEN
    );
    let resp2 = client.get(&encoded_url).send().await.unwrap();
    assert!(resp2.status() == StatusCode::NOT_FOUND || resp2.status() == StatusCode::FORBIDDEN);

    // 3. Arbitrary route attempt
    let route_url = format!("http://{}/secret_file.mp4?token={}", server.local_addr, VALID_TOKEN);
    let resp3 = client.get(&route_url).send().await.unwrap();
    assert_eq!(resp3.status(), StatusCode::NOT_FOUND);

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_lifecycle_post_shutdown_rejection() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 1000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let stream_url = server.stream_url();

    // Verify active streaming works
    let resp_before = client.get(&stream_url).header(RANGE, "bytes=0-10").send().await.unwrap();
    assert_eq!(resp_before.status(), StatusCode::PARTIAL_CONTENT);

    // Close room server
    server.shutdown().await;

    // Subsequent request must be rejected with 403 or connection error
    let resp_after = client.get(&stream_url).header(RANGE, "bytes=0-10").send().await;
    match resp_after {
        Ok(resp) => assert_eq!(resp.status(), StatusCode::FORBIDDEN),
        Err(e) => assert!(e.is_connect()),
    }
}

#[tokio::test]
async fn test_tier2_concurrent_byte_range_streaming() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_bytes = Arc::new(create_synthetic_media_file(temp_file.path(), 512 * 1024).expect("Write file"));

    let server = Arc::new(TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap());
    let client = reqwest::Client::new();

    let mut handles = Vec::new();
    // Spawn 10 concurrent requests for different 16 KB segments
    for i in 0..10 {
        let client_clone = client.clone();
        let stream_url = server.stream_url();
        let file_bytes_clone = Arc::clone(&file_bytes);
        let start = i * 16384;
        let end = start + 16383;

        handles.push(tokio::spawn(async move {
            let resp = client_clone
                .get(&stream_url)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .expect("Concurrent request failed");

            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let bytes = resp.bytes().await.expect("Read bytes failed");
            assert_eq!(bytes.len(), 16384);
            assert_eq!(
                compute_sha256(&bytes),
                compute_range_sha256(&file_bytes_clone, start, end)
            );
        }));
    }

    for handle in handles {
        handle.await.expect("Task panicked");
    }

    let server_handle = Arc::try_unwrap(server).unwrap_or_else(|_| panic!("Failed to unwrap server Arc"));
    server_handle.shutdown().await;
}

// Additional Tier 1 Feature Coverage

#[tokio::test]
async fn test_tier1_streaming_head_request_status_and_headers() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    create_synthetic_media_file(temp_file.path(), 5000).expect("Write file");

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let resp = client.head(server.stream_url()).send().await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "5000");
    assert_eq!(resp.headers().get(ACCEPT_RANGES).unwrap(), "bytes");
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 0);

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_streaming_mkv_content_type_header() {
    let dir = tempfile::tempdir().unwrap();
    let mkv_path = dir.path().join("movie.mkv");
    create_synthetic_media_file(&mkv_path, 1024).unwrap();

    let server = TestMediaServer::start(&mkv_path, VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let resp = client.get(server.stream_url()).send().await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get(CONTENT_TYPE).unwrap(), "video/x-matroska");

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_streaming_mov_content_type_header() {
    let dir = tempfile::tempdir().unwrap();
    let mov_path = dir.path().join("sample.mov");
    create_synthetic_media_file(&mov_path, 1024).unwrap();

    let server = TestMediaServer::start(&mov_path, VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let resp = client.get(server.stream_url()).send().await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get(CONTENT_TYPE).unwrap(), "video/quicktime");

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_streaming_cache_control_no_cache_header() {
    let temp_file = NamedTempFile::new().unwrap();
    create_synthetic_media_file(temp_file.path(), 1024).unwrap();

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let resp = client.get(server.stream_url()).send().await.unwrap();

    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-cache, private");

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier1_streaming_exact_full_file_range_header() {
    let temp_file = NamedTempFile::new().unwrap();
    let size = 2048;
    let file_bytes = create_synthetic_media_file(temp_file.path(), size).unwrap();

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let resp = client
        .get(server.stream_url())
        .header(RANGE, format!("bytes=0-{}", size - 1))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), &size.to_string());
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), &format!("bytes 0-{}/{}", size - 1, size));

    let body = resp.bytes().await.unwrap();
    assert_eq!(compute_sha256(&body), compute_sha256(&file_bytes));

    server.shutdown().await;
}

// Additional Tier 2 Boundary Coverage

#[tokio::test]
async fn test_tier2_range_suffix_larger_than_total_file_size() {
    let temp_file = NamedTempFile::new().unwrap();
    let file_bytes = create_synthetic_media_file(temp_file.path(), 1000).unwrap();

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    // RFC 9110: If suffix length exceeds file length, start is 0
    let resp = client
        .get(server.stream_url())
        .header(RANGE, "bytes=-5000")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes 0-999/1000");
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1000");

    let body = resp.bytes().await.unwrap();
    assert_eq!(compute_sha256(&body), compute_sha256(&file_bytes));

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_auth_token_with_whitespace_trimmed() {
    let temp_file = NamedTempFile::new().unwrap();
    create_synthetic_media_file(temp_file.path(), 1024).unwrap();

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let url = format!("http://{}/video/stream", server.local_addr);

    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer   {}   ", VALID_TOKEN))
        .header(RANGE, "bytes=0-10")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_auth_case_sensitivity_constant_time() {
    let temp_file = NamedTempFile::new().unwrap();
    create_synthetic_media_file(temp_file.path(), 1024).unwrap();

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    // Token with uppercase character when token is lowercase
    let upper_token = VALID_TOKEN.to_uppercase();
    let url = format!("http://{}/video/stream?token={}", server.local_addr, upper_token);

    let resp = client.get(&url).header(RANGE, "bytes=0-10").send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_security_encoded_null_byte_path_traversal() {
    let temp_file = NamedTempFile::new().unwrap();
    create_synthetic_media_file(temp_file.path(), 1024).unwrap();

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let null_url = format!("http://{}/video/stream%00/etc/passwd?token={}", server.local_addr, VALID_TOKEN);
    let resp = client.get(&null_url).send().await.unwrap();

    assert!(resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::BAD_REQUEST);
    server.shutdown().await;
}

#[tokio::test]
async fn test_tier2_security_backslash_traversal_rejected() {
    let temp_file = NamedTempFile::new().unwrap();
    create_synthetic_media_file(temp_file.path(), 1024).unwrap();

    let server = TestMediaServer::start(temp_file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();

    let backslash_url = format!("http://{}/video/stream/..\\..\\windows\\system32?token={}", server.local_addr, VALID_TOKEN);
    let resp = client.get(&backslash_url).send().await.unwrap();

    assert!(resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::BAD_REQUEST);
    server.shutdown().await;
}
