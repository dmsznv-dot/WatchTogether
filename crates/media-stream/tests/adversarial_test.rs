use media_stream::MediaServer;
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use std::io::Write;
use tempfile::NamedTempFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const VALID_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"; // 64 chars
const TEST_FILE_SIZE: usize = 512 * 1024; // 512 KB

fn create_test_file(size: usize) -> (NamedTempFile, Vec<u8>) {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let mut data = Vec::with_capacity(size);
    for i in 0..size {
        data.push(((i * 13 + 37) % 251) as u8);
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

// =========================================================================
// 1. Path Traversal & Isolation Adversarial Tests
// =========================================================================

#[tokio::test]
async fn test_path_traversal_via_raw_tcp() {
    let (file, _) = create_test_file(1024);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let addr = server.local_addr;

    // Send raw unnormalized HTTP GET requests over raw TCP socket
    let traversal_paths = [
        "/video/stream/../../../etc/passwd",
        "/video/stream/../../../../etc/shadow",
        "/video/stream/%2e%2e/%2e%2e/etc/passwd",
        "/%2e%2e/%2e%2e/%2e%2e/etc/passwd",
        "/etc/passwd",
        "/other/file.txt",
        "/video/stream/..",
        "/video/stream.",
        "/video/stream/extra",
        "/video/stream?token=",
    ];

    for path in traversal_paths {
        let mut stream = TcpStream::connect(addr).await.expect("Failed to connect via TCP");
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            path, addr
        );
        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let resp_str = String::from_utf8_lossy(&response);

        // All non-exact /video/stream paths must be 404 Not Found or 400 Bad Request or 403 Forbidden
        // NONE should return 200 or 206
        assert!(
            !resp_str.starts_with("HTTP/1.1 200") && !resp_str.starts_with("HTTP/1.1 206"),
            "Path '{}' returned success instead of rejection: {}",
            path,
            resp_str
        );

        if path.starts_with("/video/stream?token=") {
            assert!(
                resp_str.starts_with("HTTP/1.1 403"),
                "Expected 403 for empty token, got: {}",
                resp_str
            );
        } else {
            assert!(
                resp_str.starts_with("HTTP/1.1 404") || resp_str.starts_with("HTTP/1.1 400"),
                "Expected 404 or 400 for path '{}', got: {}",
                path,
                resp_str
            );
        }
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_auth_token_edge_cases_and_boundaries() {
    let (file, _) = create_test_file(1024);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let base_url = format!("http://{}/video/stream", server.local_addr);

    // 1. Missing token query param & missing header
    let resp = client.get(&base_url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 2. Empty token in query param
    let resp = client.get(format!("{}?token=", base_url)).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 3. Truncated 31-character token (1 char short of 32-char CSPRNG requirement)
    let token_31 = &VALID_TOKEN[0..31];
    let resp = client.get(format!("{}?token={}", base_url, token_31)).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 4. Token with 1 byte altered (bit flip)
    let mut altered_token = VALID_TOKEN.to_string();
    altered_token.replace_range(10..11, "X");
    let resp = client.get(format!("{}?token={}", base_url, altered_token)).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 5. Huge token (10,000 characters) - DOS prevention
    let huge_token = "A".repeat(10000);
    let resp = client.get(format!("{}?token={}", base_url, huge_token)).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 6. Bearer header empty or whitespace
    let resp = client
        .get(&base_url)
        .header(AUTHORIZATION, "Bearer ")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let resp = client
        .get(&base_url)
        .header(AUTHORIZATION, "Bearer    ")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 7. Non-Bearer authorization scheme
    let resp = client
        .get(&base_url)
        .header(AUTHORIZATION, format!("Basic {}", VALID_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 8. Valid Bearer authorization succeeds
    let resp = client
        .get(&base_url)
        .header(AUTHORIZATION, format!("Bearer {}", VALID_TOKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    server.shutdown().await.unwrap();
}

// =========================================================================
// 2. Range Boundary Edge Cases Adversarial Tests
// =========================================================================

#[tokio::test]
async fn test_zero_byte_file_behavior() {
    let (file, _) = create_test_file(0);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let url = server.stream_url();

    // 1. Full content on 0-byte file: 200 OK with Content-Length: 0
    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "0");
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 0);

    // 2. Range request on 0-byte file: bytes=0-0 must return 416
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes */0");

    // 3. Open range on 0-byte file: bytes=0- must return 416
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=0-")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes */0");

    // 4. Suffix range on 0-byte file: bytes=-1 must return 416
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=-1")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes */0");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_one_byte_file_boundaries() {
    let (file, data) = create_test_file(1);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let url = server.stream_url();

    // 1. bytes=0-0 on 1-byte file -> 206 Partial Content, Content-Length 1
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes 0-0/1");
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1");
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0], data[0]);

    // 2. bytes=0-10 on 1-byte file -> clamped to 0-0/1
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=0-10")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes 0-0/1");
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1");
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0], data[0]);

    // 3. bytes=1-1 on 1-byte file -> 416 Range Not Satisfiable
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=1-1")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes */1");

    // 4. bytes=1- on 1-byte file -> 416 Range Not Satisfiable
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=1-")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes */1");

    // 5. bytes=-1 on 1-byte file -> 206, bytes 0-0/1
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=-1")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers().get(CONTENT_RANGE).unwrap(), "bytes 0-0/1");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_range_parser_adversarial_matrix() {
    let (file, data) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let client = reqwest::Client::new();
    let url = server.stream_url();

    // 1. Inverted range bytes=100-50 -> 416 Range Not Satisfiable
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=100-50")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes */{}", TEST_FILE_SIZE)
    );

    // 2. bytes=total- where start == total_size -> 416
    let resp = client
        .get(&url)
        .header(RANGE, format!("bytes={}-", TEST_FILE_SIZE))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);

    // 3. bytes=total+100- -> 416
    let resp = client
        .get(&url)
        .header(RANGE, format!("bytes={}-", TEST_FILE_SIZE + 100))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);

    // 4. bytes=-0 suffix length 0 -> 400 Bad Request
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=-0")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 5. Suffix larger than file bytes=-1000000 -> clamped to whole file (RFC 9110 §14.1.2)
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=-1000000")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes 0-{}/{}", TEST_FILE_SIZE - 1, TEST_FILE_SIZE)
    );
    assert_eq!(
        resp.headers().get(CONTENT_LENGTH).unwrap(),
        &TEST_FILE_SIZE.to_string()
    );

    // 6. Single byte range at exact boundary: bytes=0-0 and bytes=(total-1)-(total-1)
    let resp = client
        .get(&url)
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes 0-0/{}", TEST_FILE_SIZE)
    );
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1");
    let b = resp.bytes().await.unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0], data[0]);

    let last_idx = TEST_FILE_SIZE - 1;
    let resp = client
        .get(&url)
        .header(RANGE, format!("bytes={}-{}", last_idx, last_idx))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(CONTENT_RANGE).unwrap(),
        &format!("bytes {}-{}/{}", last_idx, last_idx, TEST_FILE_SIZE)
    );
    assert_eq!(resp.headers().get(CONTENT_LENGTH).unwrap(), "1");
    let b = resp.bytes().await.unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0], data[last_idx]);

    // 7. Malformed range syntax -> 400 Bad Request
    let malformed_headers = [
        "bytes=-",
        "bytes=---",
        "bytes=1-2-3",
        "bytes=abc-def",
        "bytes=100-abc",
        "bytes=abc-100",
        "bytes=100-200,300-400", // multiple ranges rejected
        "bytes=100 200",
    ];
    for bad in malformed_headers {
        let resp = client.get(&url).header(RANGE, bad).send().await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "Header '{}' did not produce 400 Bad Request",
            bad
        );
    }

    // 8. Unsupported unit (e.g. seconds=0-10) -> RFC 9110 says ignore unrecognized unit and serve 200 OK
    let resp = client
        .get(&url)
        .header(RANGE, "seconds=0-10")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(CONTENT_LENGTH).unwrap(),
        &TEST_FILE_SIZE.to_string()
    );

    server.shutdown().await.unwrap();
}

// =========================================================================
// 3. Concurrent Load & Rapid Stream Requests
// =========================================================================

#[tokio::test]
async fn test_rapid_concurrent_load_over_loopback() {
    let (file, data) = create_test_file(TEST_FILE_SIZE);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let url = server.stream_url();
    let data_arc = std::sync::Arc::new(data);

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(50)
        .build()
        .unwrap();

    let mut tasks = Vec::new();
    let num_requests = 100;

    for i in 0..num_requests {
        let client_clone = client.clone();
        let url_clone = url.clone();
        let data_ref = data_arc.clone();

        let task = tokio::spawn(async move {
            let chunk_size = 4096;
            let max_start = TEST_FILE_SIZE - chunk_size;
            let start = (i * 7919) % max_start;
            let end = start + chunk_size - 1;

            let resp = client_clone
                .get(&url_clone)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .expect("Concurrent request failed");

            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(
                resp.headers().get(CONTENT_RANGE).unwrap(),
                &format!("bytes {}-{}/{}", start, end, TEST_FILE_SIZE)
            );

            let body = resp.bytes().await.expect("Failed to read body");
            assert_eq!(body.len(), chunk_size);
            assert_eq!(
                sha256_digest(&body),
                sha256_digest(&data_ref[start..=end]),
                "Data slice checksum mismatch at request {}",
                i
            );
        });

        tasks.push(task);
    }

    for task in tasks {
        task.await.expect("Task panicked during concurrent load test");
    }

    server.shutdown().await.unwrap();
}

// =========================================================================
// 4. Server Teardown & Lifecycle Verification
// =========================================================================

#[tokio::test]
async fn test_server_teardown_and_post_shutdown_behavior() {
    let (file, _) = create_test_file(1024);
    let server = MediaServer::start_local(file.path(), VALID_TOKEN).await.unwrap();
    let url = server.stream_url();
    let client = reqwest::Client::new();

    // 1. Initial request works
    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Shut down server
    server.shutdown().await.unwrap();

    // 3. Post-shutdown requests: must either be connection refused (OS level) or 403 Forbidden
    let post_shutdown = client.get(&url).send().await;
    match post_shutdown {
        Ok(r) => {
            assert!(
                r.status() == StatusCode::FORBIDDEN || !r.status().is_success(),
                "Post-shutdown request succeeded unexpectedly: {}",
                r.status()
            );
        }
        Err(e) => {
            assert!(
                e.is_connect() || e.is_request(),
                "Expected connection refused or request error, got: {:?}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_invalid_file_configurations() {
    // 1. Non-existent file
    let bad_path = std::path::PathBuf::from("/non_existent_dir_12345/video.mp4");
    let result = MediaServer::start_local(&bad_path, VALID_TOKEN).await;
    assert!(result.is_err(), "Must fail on non-existent file");

    // 2. Directory path instead of regular file
    let dir = tempfile::tempdir().unwrap();
    let result = MediaServer::start_local(dir.path(), VALID_TOKEN).await;
    assert!(result.is_err(), "Must fail when passed a directory");
}
