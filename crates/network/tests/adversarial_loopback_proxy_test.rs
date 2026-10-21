use network::{
    LoopbackProxy, LoopbackProxyConfig, MockP2pTransport, P2pTransport, TunnelResponse,
};
use reqwest::header::RANGE;
use reqwest::StatusCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

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
async fn test_adversarial_massive_scrambled_demux_50_streams() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    const NUM_STREAMS: usize = 50;
    const CHUNK_SIZE: usize = 1024;
    const TOTAL_PAYLOAD: usize = NUM_STREAMS * CHUNK_SIZE;

    // Deterministic pseudo-random payload
    let payload: Vec<u8> = (0..TOTAL_PAYLOAD)
        .map(|i| ((i * 73 + 131) ^ (i >> 3)) as u8)
        .collect();
    let payload_arc = Arc::new(payload);

    // Host responder: receives all 50 requests, scrambles their order using a permutation,
    // adds synthetic jitter to simulate variable network/disk latency, and sends responses out.
    let payload_host = payload_arc.clone();
    let host_handle = tokio::spawn(async move {
        let mut requests = Vec::with_capacity(NUM_STREAMS);
        for _ in 0..NUM_STREAMS {
            let req = t_host.recv_media_request().await.expect("recv req");
            requests.push(req);
        }

        // Adversarially permute requests: reverse order with odd/even interleaving
        let mut permuted = Vec::with_capacity(NUM_STREAMS);
        let mut evens = Vec::new();
        let mut odds = Vec::new();
        for (i, r) in requests.into_iter().enumerate() {
            if i % 2 == 0 {
                evens.push(r);
            } else {
                odds.push(r);
            }
        }
        // Interleave odds reversed, then evens reversed
        for r in odds.into_iter().rev() {
            permuted.push(r);
        }
        for r in evens.into_iter().rev() {
            permuted.push(r);
        }

        let mut join_set = tokio::task::JoinSet::new();
        for (idx, req) in permuted.into_iter().enumerate() {
            let t_host_worker = t_host.clone();
            let p_ref = payload_host.clone();
            join_set.spawn(async move {
                // Introduce variable jitter: 0 to 10 ms depending on index
                let jitter_ms = (idx * 3 % 11) as u64;
                if jitter_ms > 0 {
                    sleep(Duration::from_millis(jitter_ms)).await;
                }

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

                let chunk_data = p_ref[start..=end].to_vec();
                let resp = TunnelResponse {
                    request_id: req.request_id,
                    status: 206,
                    headers: vec![
                        (
                            "content-range".into(),
                            format!("bytes {}-{}/{}", start, end, TOTAL_PAYLOAD),
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
            res.expect("Worker task finished cleanly");
        }
    });

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let client = reqwest::Client::new();
    let stream_url = proxy_handle.stream_url("adversarial_demux_token");

    let mut tasks = Vec::new();
    for i in 0..NUM_STREAMS {
        let client_c = client.clone();
        let url_c = stream_url.clone();
        let payload_ref = payload_arc.clone();

        let start = i * CHUNK_SIZE;
        let end = start + CHUNK_SIZE - 1;
        let expected_chunk = payload_ref[start..=end].to_vec();
        let expected_hash = sha256(&expected_chunk);

        tasks.push(tokio::spawn(async move {
            let resp = client_c
                .get(&url_c)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .expect("HTTP send failed");

            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let expected_cr = format!("bytes {}-{}/{}", start, end, TOTAL_PAYLOAD);
            let actual_cr = resp
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            assert_eq!(actual_cr, expected_cr, "Content-Range mismatch on stream {}", i);

            let body = resp.bytes().await.expect("read body");
            let actual_hash = sha256(&body);
            assert_eq!(
                actual_hash, expected_hash,
                "SHA-256 data corruption on stream {}",
                i
            );
        }));
    }

    for task in tasks {
        task.await.expect("Client task panicked");
    }

    host_handle.await.expect("Host completed");
    proxy_handle.shutdown().await.expect("Proxy shutdown cleanly");
}

#[tokio::test]
async fn test_adversarial_bogus_and_duplicate_request_id_injection() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    let host_handle = tokio::spawn(async move {
        let req = t_host.recv_media_request().await.expect("recv req");

        // 1. Inject completely bogus request_id that does not exist
        let bogus_resp = TunnelResponse {
            request_id: 999_999_999,
            status: 200,
            headers: vec![("x-bogus".into(), "true".into())],
            body: b"bogus payload".to_vec(),
        };
        t_host.send_media_response(bogus_resp).await.expect("send bogus");

        // 2. Send the legitimate response
        let legit_resp = TunnelResponse {
            request_id: req.request_id,
            status: 206,
            headers: vec![
                ("content-range".into(), "bytes 0-9/10".into()),
                ("content-length".into(), "10".into()),
            ],
            body: b"0123456789".to_vec(),
        };
        t_host.send_media_response(legit_resp.clone()).await.expect("send legit");

        // 3. Inject duplicate response for the same request_id (already consumed)
        t_host.send_media_response(legit_resp).await.expect("send duplicate");
    });

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let client = reqwest::Client::new();
    let stream_url = proxy_handle.stream_url("bogus_test_token");

    let resp = client
        .get(&stream_url)
        .header(RANGE, "bytes=0-9")
        .send()
        .await
        .expect("HTTP request");

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get("content-range").unwrap().to_str().unwrap(),
        "bytes 0-9/10"
    );
    let body = resp.bytes().await.expect("read body");
    assert_eq!(body.as_ref(), b"0123456789");

    host_handle.await.expect("Host completed");
    proxy_handle.shutdown().await.expect("Proxy shutdown cleanly");
}

#[tokio::test]
async fn test_adversarial_transport_disconnect_mid_flight_returns_502() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    let proxy_handle = LoopbackProxy::start(t_client.clone(), LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let stream_url = proxy_handle.stream_url("disconnect_token");
    let client = reqwest::Client::new();

    // Spawn a task that receives the request and immediately drops / closes the transport
    let host_handle = tokio::spawn(async move {
        let _req = t_host.recv_media_request().await.expect("recv req");
        // Forcibly close transport before answering
        t_host.close().await.expect("close transport");
    });

    let resp = client
        .get(&stream_url)
        .header(RANGE, "bytes=0-100")
        .send()
        .await
        .expect("HTTP request completes");

    // The client should receive BAD_GATEWAY (502) because the tunnel closed before response
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);

    host_handle.await.expect("Host task finished");
    proxy_handle.shutdown().await.expect("Proxy shutdown");
}

#[tokio::test]
async fn test_adversarial_shutdown_under_active_in_flight_load() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    // Host that does not respond (stalls)
    let host_handle = tokio::spawn(async move {
        for _ in 0..10 {
            if let Ok(req) = t_host.recv_media_request().await {
                // Do not respond; just acknowledge receipt
                let _ = req.request_id;
            }
        }
    });

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let stream_url = proxy_handle.stream_url("shutdown_load_token");
    let client = reqwest::Client::new();

    let counter = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();

    for _ in 0..10 {
        let c = client.clone();
        let u = stream_url.clone();
        let cnt = counter.clone();
        tasks.push(tokio::spawn(async move {
            let res = c.get(&u).header(RANGE, "bytes=0-100").send().await;
            if res.is_ok() {
                cnt.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    // Give requests time to reach proxy and enter pending state
    sleep(Duration::from_millis(50)).await;

    // Now trigger shutdown while requests are in flight
    proxy_handle.shutdown().await.expect("Shutdown cleanly under load");

    // Join all client tasks; they should terminate without hanging
    for t in tasks {
        let _ = t.await;
    }

    let _ = host_handle.await;
}

#[tokio::test]
async fn test_adversarial_port_release_after_shutdown() {
    let (t_client, _t_host) = MockP2pTransport::create_pair();

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let addr = proxy_handle.local_addr;
    let url = format!("http://{}/video/stream?token=release_test", addr);

    // Shutdown proxy
    proxy_handle.shutdown().await.expect("Proxy shutdown");

    // Attempting to connect immediately to the shutdown address should fail (connection refused)
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();

    let res = client.get(&url).send().await;
    assert!(
        res.is_err(),
        "Expected connection to fail after proxy shutdown, but got {:?}",
        res
    );
}

#[tokio::test]
async fn test_adversarial_fallback_route_returns_404() {
    let (t_client, _t_host) = MockP2pTransport::create_pair();

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let client = reqwest::Client::new();

    // Query unmapped routes
    let paths = [
        "/",
        "/video",
        "/video/stream_extra",
        "/admin/secrets",
        "/etc/passwd",
    ];
    for path in paths {
        let url = format!("http://{}{}", proxy_handle.local_addr, path);
        let resp = client.get(&url).send().await.expect("send request");
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "Path '{}' must return 404",
            path
        );
    }

    proxy_handle.shutdown().await.expect("Proxy shutdown");
}
