use network::{
    LoopbackProxy, LoopbackProxyConfig, MockP2pTransport, P2pTransport, TunnelResponse,
};
use reqwest::header::RANGE;
use reqwest::StatusCode;
use std::sync::Arc;

#[allow(clippy::chunks_exact_to_as_chunks)]
fn sha256(data: &[u8]) -> [u8; 32] {
    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, c) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h_val) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_val
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h_val = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_val);
    }
    let mut out = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

#[tokio::test]
async fn test_sha256_self_test() {
    // SHA-256 of empty string
    assert_eq!(
        sha256(b""),
        [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55
        ]
    );
    // SHA-256 of "hello"
    assert_eq!(
        sha256(b"hello"),
        [
            0x2c, 0xf2, 0x4d, 0xba, 0x5f, 0xb0, 0xa3, 0x0e, 0x26, 0xe8, 0x3b, 0x2a, 0xc5, 0xb9,
            0xe2, 0x9e, 0x1b, 0x16, 0x1e, 0x5c, 0x1f, 0xa7, 0x42, 0x5e, 0x73, 0x04, 0x33, 0x62,
            0x93, 0x8b, 0x98, 0x24
        ]
    );
}

#[tokio::test]
async fn test_loopback_proxy_concurrent_byte_range_requests_fifo() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    // Prepare a 20KB test payload (20 chunks of 1024 bytes)
    const CHUNK_SIZE: usize = 1024;
    const NUM_CHUNKS: usize = 20;
    const TOTAL_LEN: usize = CHUNK_SIZE * NUM_CHUNKS;

    let payload: Vec<u8> = (0..TOTAL_LEN).map(|i| ((i * 31 + 7) % 256) as u8).collect();
    let payload_arc = Arc::new(payload);

    // Host responder: processes requests in FIFO order
    let payload_host = payload_arc.clone();
    let host_handle = tokio::spawn(async move {
        for _ in 0..NUM_CHUNKS {
            let req = t_host.recv_media_request().await.expect("recv req");
            let range_header = req
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("range"))
                .map(|(_, v)| v.clone())
                .expect("Range header present");

            // Parse bytes=start-end
            let range_str = range_header.strip_prefix("bytes=").unwrap();
            let parts: Vec<&str> = range_str.split('-').collect();
            let start: usize = parts[0].parse().unwrap();
            let end: usize = parts[1].parse().unwrap();

            let chunk_data = payload_host[start..=end].to_vec();
            let resp = TunnelResponse {
                request_id: req.request_id,
                status: 206,
                headers: vec![
                    (
                        "content-range".into(),
                        format!("bytes {}-{}/{}", start, end, TOTAL_LEN),
                    ),
                    ("content-length".into(), format!("{}", chunk_data.len())),
                    ("content-type".into(), "video/mp4".into()),
                    ("accept-ranges".into(), "bytes".into()),
                ],
                body: chunk_data,
            };
            t_host.send_media_response(resp).await.expect("send resp");
        }
    });

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let client = reqwest::Client::new();
    let stream_url = proxy_handle.stream_url("concurrent_token_test");

    // Launch NUM_CHUNKS concurrent requests
    let mut tasks = Vec::new();
    for i in 0..NUM_CHUNKS {
        let client_clone = client.clone();
        let url_clone = stream_url.clone();
        let payload_ref = payload_arc.clone();

        let start = i * CHUNK_SIZE;
        let end = start + CHUNK_SIZE - 1;
        let expected_chunk = payload_ref[start..=end].to_vec();
        let expected_hash = sha256(&expected_chunk);

        tasks.push(tokio::spawn(async move {
            let resp = client_clone
                .get(&url_clone)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .expect("HTTP send failed");

            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let expected_cr = format!("bytes {}-{}/{}", start, end, TOTAL_LEN);
            let actual_cr = resp
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            assert_eq!(actual_cr, expected_cr, "Content-Range header mismatch");

            let body = resp.bytes().await.expect("read body");
            let actual_hash = sha256(&body);
            assert_eq!(
                actual_hash, expected_hash,
                "SHA-256 data corruption on chunk {} (range {}-{})",
                i, start, end
            );
        }));
    }

    for task in tasks {
        task.await.expect("Task panicked");
    }

    host_handle.await.expect("Host finished");
    proxy_handle.shutdown().await.expect("Proxy shutdown");
}

#[tokio::test]
async fn test_loopback_proxy_concurrent_byte_range_requests_out_of_order_host() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    // Prepare a 10KB test payload (10 chunks of 1024 bytes)
    const CHUNK_SIZE: usize = 1024;
    const NUM_CHUNKS: usize = 10;
    const TOTAL_LEN: usize = CHUNK_SIZE * NUM_CHUNKS;

    let payload: Vec<u8> = (0..TOTAL_LEN).map(|i| ((i * 17 + 3) % 256) as u8).collect();
    let payload_arc = Arc::new(payload);

    // Host responder: receives all requests, but responds with slight delay or reordered
    let payload_host = payload_arc.clone();
    let host_handle = tokio::spawn(async move {
        // Collect all requests first
        let mut reqs = Vec::new();
        for _ in 0..NUM_CHUNKS {
            let req = t_host.recv_media_request().await.expect("recv req");
            reqs.push(req);
        }

        // Respond in REVERSE order (simulating asynchronous disk seeks / caching)
        for req in reqs.into_iter().rev() {
            let range_header = req
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("range"))
                .map(|(_, v)| v.clone())
                .expect("Range header present");

            let range_str = range_header.strip_prefix("bytes=").unwrap();
            let parts: Vec<&str> = range_str.split('-').collect();
            let start: usize = parts[0].parse().unwrap();
            let end: usize = parts[1].parse().unwrap();

            let chunk_data = payload_host[start..=end].to_vec();
            let resp = TunnelResponse {
                request_id: req.request_id,
                status: 206,
                headers: vec![
                    (
                        "content-range".into(),
                        format!("bytes {}-{}/{}", start, end, TOTAL_LEN),
                    ),
                    ("content-length".into(), format!("{}", chunk_data.len())),
                    ("content-type".into(), "video/mp4".into()),
                    ("accept-ranges".into(), "bytes".into()),
                ],
                body: chunk_data,
            };
            t_host.send_media_response(resp).await.expect("send resp");
        }
    });

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let client = reqwest::Client::new();
    let stream_url = proxy_handle.stream_url("out_of_order_test");

    let mut tasks = Vec::new();
    for i in 0..NUM_CHUNKS {
        let client_clone = client.clone();
        let url_clone = stream_url.clone();
        let payload_ref = payload_arc.clone();

        let start = i * CHUNK_SIZE;
        let end = start + CHUNK_SIZE - 1;
        let expected_chunk = payload_ref[start..=end].to_vec();
        let expected_hash = sha256(&expected_chunk);

        tasks.push(tokio::spawn(async move {
            let resp = client_clone
                .get(&url_clone)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .expect("HTTP send failed");

            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let expected_cr = format!("bytes {}-{}/{}", start, end, TOTAL_LEN);
            let actual_cr = resp
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            assert_eq!(actual_cr, expected_cr, "Content-Range header mismatch");

            let body = resp.bytes().await.expect("read body");
            let actual_hash = sha256(&body);
            assert_eq!(
                actual_hash, expected_hash,
                "SHA-256 data corruption on chunk {} (range {}-{})",
                i, start, end
            );
        }));
    }

    for task in tasks {
        task.await.expect("Task panicked");
    }

    host_handle.await.expect("Host finished");
    proxy_handle.shutdown().await.expect("Proxy shutdown");
}

#[tokio::test]
async fn test_loopback_proxy_concurrent_async_host_task_interleaving() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    const CHUNK_SIZE: usize = 1024;
    const NUM_CHUNKS: usize = 16;
    const TOTAL_LEN: usize = CHUNK_SIZE * NUM_CHUNKS;

    let payload: Vec<u8> = (0..TOTAL_LEN).map(|i| ((i * 43 + 19) % 256) as u8).collect();
    let payload_arc = Arc::new(payload);

    // Host responder: handles each request in a spawned task with natural async concurrency
    let payload_host = payload_arc.clone();
    let t_host_clone = t_host.clone();
    let host_handle = tokio::spawn(async move {
        let mut join_set = tokio::task::JoinSet::new();
        for _ in 0..NUM_CHUNKS {
            let req = t_host_clone.recv_media_request().await.expect("recv req");
            let payload_ref = payload_host.clone();
            let t_host_worker = t_host_clone.clone();

            join_set.spawn(async move {
                let range_header = req
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("range"))
                    .map(|(_, v)| v.clone())
                    .expect("Range header present");

                let range_str = range_header.strip_prefix("bytes=").unwrap();
                let parts: Vec<&str> = range_str.split('-').collect();
                let start: usize = parts[0].parse().unwrap();
                let end: usize = parts[1].parse().unwrap();

                // Small jitter (0-2ms) simulating variable disk read times
                let jitter_ms = (start % 3) as u64;
                if jitter_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(jitter_ms)).await;
                }

                let chunk_data = payload_ref[start..=end].to_vec();
                let resp = TunnelResponse {
                    request_id: req.request_id,
                    status: 206,
                    headers: vec![
                        (
                            "content-range".into(),
                            format!("bytes {}-{}/{}", start, end, TOTAL_LEN),
                        ),
                        ("content-length".into(), format!("{}", chunk_data.len())),
                        ("content-type".into(), "video/mp4".into()),
                        ("accept-ranges".into(), "bytes".into()),
                    ],
                    body: chunk_data,
                };
                t_host_worker
                    .send_media_response(resp)
                    .await
                    .expect("send resp");
            });
        }

        while let Some(res) = join_set.join_next().await {
            res.expect("Worker panicked");
        }
    });

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let client = reqwest::Client::new();
    let stream_url = proxy_handle.stream_url("async_interleaving_test");

    let mut tasks = Vec::new();
    for i in 0..NUM_CHUNKS {
        let client_clone = client.clone();
        let url_clone = stream_url.clone();
        let payload_ref = payload_arc.clone();

        let start = i * CHUNK_SIZE;
        let end = start + CHUNK_SIZE - 1;
        let expected_chunk = payload_ref[start..=end].to_vec();
        let expected_hash = sha256(&expected_chunk);

        tasks.push(tokio::spawn(async move {
            let resp = client_clone
                .get(&url_clone)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .expect("HTTP send failed");

            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let expected_cr = format!("bytes {}-{}/{}", start, end, TOTAL_LEN);
            let actual_cr = resp
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            assert_eq!(actual_cr, expected_cr, "Content-Range header mismatch");

            let body = resp.bytes().await.expect("read body");
            let actual_hash = sha256(&body);
            assert_eq!(
                actual_hash, expected_hash,
                "SHA-256 data corruption on chunk {} (range {}-{})",
                i, start, end
            );
        }));
    }

    for task in tasks {
        task.await.expect("Task panicked");
    }

    host_handle.await.expect("Host finished");
    proxy_handle.shutdown().await.expect("Proxy shutdown");
}

