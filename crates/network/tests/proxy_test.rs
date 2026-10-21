use network::{
    LoopbackProxy, LoopbackProxyConfig, MockP2pTransport, P2pTransport, TunnelResponse,
};
use reqwest::header::RANGE;
use reqwest::StatusCode;

#[tokio::test]
async fn test_loopback_proxy_tunnels_http_range_requests() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    // Spawn simulated host responder
    let host_handle = tokio::spawn(async move {
        let req = t_host.recv_media_request().await.expect("Receive req");
        assert_eq!(req.method, "GET");
        assert!(req.uri.contains("/video/stream?token=test_token_123"));

        let has_range = req
            .headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("range") && v == "bytes=0-4");
        assert!(has_range, "Headers must include Range: bytes=0-4");

        let resp = TunnelResponse {
            request_id: req.request_id,
            status: 206,
            headers: vec![
                ("content-range".into(), "bytes 0-4/100".into()),
                ("content-length".into(), "5".into()),
                ("content-type".into(), "video/mp4".into()),
                ("accept-ranges".into(), "bytes".into()),
            ],
            body: vec![10, 20, 30, 40, 50],
        };
        t_host.send_media_response(resp).await.expect("Send resp");
    });

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let client = reqwest::Client::new();
    let stream_url = proxy_handle.stream_url("test_token_123");

    let resp = client
        .get(&stream_url)
        .header(RANGE, "bytes=0-4")
        .send()
        .await
        .expect("Send HTTP request");

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get("content-range").unwrap().to_str().unwrap(),
        "bytes 0-4/100"
    );
    let bytes = resp.bytes().await.expect("Read response bytes");
    assert_eq!(bytes.as_ref(), &[10, 20, 30, 40, 50]);

    host_handle.await.expect("Host task finished");

    // Test fallback route returns 404
    let not_found_url = format!("http://{}/other/path", proxy_handle.local_addr);
    let nf_resp = client.get(&not_found_url).send().await.expect("Send 404 request");
    assert_eq!(nf_resp.status(), StatusCode::NOT_FOUND);

    proxy_handle.shutdown().await.expect("Proxy shuts down");
}

#[tokio::test]
async fn test_loopback_proxy_multiplexes_concurrent_requests_correlated_by_id() {
    let (t_client, t_host) = MockP2pTransport::create_pair();

    // Spawn simulated host responder that reverses response order
    let host_handle = tokio::spawn(async move {
        let req1 = t_host.recv_media_request().await.expect("Receive req1");
        let req2 = t_host.recv_media_request().await.expect("Receive req2");

        // Send response for req2 first!
        let resp2 = TunnelResponse {
            request_id: req2.request_id,
            status: 206,
            headers: vec![
                ("content-range".into(), "bytes 100-199/1000".into()),
                ("content-length".into(), "100".into()),
                ("content-type".into(), "video/mp4".into()),
                ("accept-ranges".into(), "bytes".into()),
            ],
            body: vec![42; 100],
        };
        t_host.send_media_response(resp2).await.expect("Send resp2");

        // Then send response for req1
        let resp1 = TunnelResponse {
            request_id: req1.request_id,
            status: 206,
            headers: vec![
                ("content-range".into(), "bytes 0-99/1000".into()),
                ("content-length".into(), "100".into()),
                ("content-type".into(), "video/mp4".into()),
                ("accept-ranges".into(), "bytes".into()),
            ],
            body: vec![99; 100],
        };
        t_host.send_media_response(resp1).await.expect("Send resp1");
    });

    let proxy_handle = LoopbackProxy::start(t_client, LoopbackProxyConfig::default())
        .await
        .expect("Start proxy");

    let client = reqwest::Client::new();
    let stream_url = proxy_handle.stream_url("multi_token");

    let client1 = client.clone();
    let url1 = stream_url.clone();
    let task1 = tokio::spawn(async move {
        let resp = client1
            .get(&url1)
            .header(RANGE, "bytes=0-99")
            .send()
            .await
            .expect("Send req1");
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            resp.headers().get("content-range").unwrap().to_str().unwrap(),
            "bytes 0-99/1000"
        );
        let b = resp.bytes().await.expect("bytes1");
        assert_eq!(b.as_ref(), &[99; 100]);
    });

    let client2 = client.clone();
    let url2 = stream_url.clone();
    let task2 = tokio::spawn(async move {
        let resp = client2
            .get(&url2)
            .header(RANGE, "bytes=100-199")
            .send()
            .await
            .expect("Send req2");
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            resp.headers().get("content-range").unwrap().to_str().unwrap(),
            "bytes 100-199/1000"
        );
        let b = resp.bytes().await.expect("bytes2");
        assert_eq!(b.as_ref(), &[42; 100]);
    });

    task1.await.expect("task1 succeeded");
    task2.await.expect("task2 succeeded");
    host_handle.await.expect("host succeeded");

    proxy_handle.shutdown().await.expect("Proxy shuts down");
}

