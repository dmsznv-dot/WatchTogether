use media_engine::{HeadlessPlayer, MediaPlayer};
use network::local_sync::LocalSession;
use shared_protocol::{ClientMessage, PermissionMode};
use std::{sync::Arc, time::Duration};

async fn wait_until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(4), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn two_local_copies_follow_play_seek_pause_without_streaming() {
    let mac = Arc::new(HeadlessPlayer::new());
    mac.open_file("/mac/movies/movie.mkv").unwrap();
    let windows = Arc::new(HeadlessPlayer::new());
    windows.open_file("C:\\Movies\\movie.mkv").unwrap();
    let host = LocalSession::host(
        mac.clone(),
        "127.0.0.1:0".parse().unwrap(),
        "Mac".into(),
        PermissionMode::FreeControl,
    )
    .await
    .unwrap();
    let guest = LocalSession::join(
        windows.clone(),
        &host.endpoint,
        &host.room_code(),
        &host.token,
        "Windows".into(),
    )
    .await
    .unwrap();
    host.command(ClientMessage::PlaybackSeek {
        revision: 0,
        target_position_ms: 42000,
        client_timestamp_ms: 0,
    })
    .await
    .unwrap();
    wait_until(|| windows.get_state().unwrap().position_ms == 42000).await;
    guest
        .command(ClientMessage::PlaybackPlay {
            revision: 0,
            client_timestamp_ms: 0,
        })
        .await
        .unwrap();
    wait_until(|| mac.get_state().unwrap().is_playing && windows.get_state().unwrap().is_playing)
        .await;
    host.command(ClientMessage::PlaybackPause {
        revision: 0,
        position_ms: 43000,
        client_timestamp_ms: 0,
    })
    .await
    .unwrap();
    wait_until(|| {
        !windows.get_state().unwrap().is_playing
            && windows.get_state().unwrap().position_ms == 43000
    })
    .await;
    assert_eq!(
        windows.get_state().unwrap().current_file.as_deref(),
        Some("C:\\Movies\\movie.mkv")
    );
    assert_eq!(
        mac.get_state().unwrap().current_file.as_deref(),
        Some("/mac/movies/movie.mkv")
    );
    assert_eq!(host.snapshot().participants.len(), 2);
    host.close().await;
    wait_until(|| !guest.is_connected()).await;
    guest.close().await;
}

#[tokio::test]
async fn unreachable_host_does_not_create_a_guest_session() {
    let player = Arc::new(HeadlessPlayer::new());
    player.open_file("movie.mkv").unwrap();
    assert!(LocalSession::join(
        player,
        "ws://127.0.0.1:1/ws",
        "K7M2P9",
        &"a".repeat(64),
        "Guest".into()
    )
    .await
    .is_err());
}

#[tokio::test]
async fn host_only_blocks_guest_controls_and_close_releases_port() {
    let a = Arc::new(HeadlessPlayer::new());
    a.open_file("movie.mkv").unwrap();
    let b = Arc::new(HeadlessPlayer::new());
    b.open_file("movie.mkv").unwrap();
    let host = LocalSession::host(
        a,
        "127.0.0.1:0".parse().unwrap(),
        "Host".into(),
        PermissionMode::HostOnly,
    )
    .await
    .unwrap();
    let addr = host.listen_addr.unwrap();
    let guest = LocalSession::join(
        b,
        &host.endpoint,
        &host.room_code(),
        &host.token,
        "Guest".into(),
    )
    .await
    .unwrap();
    assert!(guest
        .command(ClientMessage::PlaybackPlay {
            revision: 0,
            client_timestamp_ms: 0
        })
        .await
        .is_err());
    guest.close().await;
    host.close().await;
    tokio::net::TcpListener::bind(addr).await.unwrap();
}

#[tokio::test]
async fn closing_host_also_closes_sockets_that_have_not_joined() {
    use futures_util::StreamExt;
    let player = Arc::new(HeadlessPlayer::new());
    player.open_file("movie.mkv").unwrap();
    let host = LocalSession::host(
        player,
        "127.0.0.1:0".parse().unwrap(),
        "Host".into(),
        PermissionMode::FreeControl,
    )
    .await
    .unwrap();
    let (mut socket, _) = tokio_tungstenite::connect_async(&host.endpoint)
        .await
        .unwrap();
    host.close().await;
    let closed = tokio::time::timeout(Duration::from_secs(2), socket.next()).await;
    assert!(
        closed.is_ok(),
        "an unjoined upgraded socket must not outlive the room"
    );
}

#[tokio::test]
async fn guest_supplied_max_revision_cannot_disable_host_controls() {
    let a=Arc::new(HeadlessPlayer::new());a.open_file("movie.mkv").unwrap();
    let b=Arc::new(HeadlessPlayer::new());b.open_file("movie.mkv").unwrap();
    let host=LocalSession::host(a.clone(),"127.0.0.1:0".parse().unwrap(),"Host".into(),PermissionMode::FreeControl).await.unwrap();
    let guest=LocalSession::join(b,&host.endpoint,&host.room_code(),&host.token,"Guest".into()).await.unwrap();
    guest.command(ClientMessage::PlaybackSeek{revision:u64::MAX,target_position_ms:12000,client_timestamp_ms:0}).await.unwrap();
    wait_until(||a.get_state().unwrap().position_ms==12000).await;
    assert!(host.snapshot().playback.revision<100);
    host.command(ClientMessage::PlaybackPause{revision:0,position_ms:13000,client_timestamp_ms:0}).await.unwrap();
    drop(host);
    wait_until(||!guest.is_connected()).await;
    guest.close().await;
}
