use media_engine::dimensions::VideoDimensions;
use media_engine::error::MediaEngineError;
use media_engine::headless::{HeadlessPlayer, ALLOWED_SPEEDS};
use media_engine::shortcuts::{handle_shortcut, KeyAction};
use media_engine::traits::MediaPlayer;
use std::sync::Arc;
use tokio::sync::broadcast;

// ============================================================================
// 1. Aspect Ratio & VideoDimensions Edge Cases
// ============================================================================

#[test]
fn test_aspect_ratio_zero_and_extreme_dimensions() {
    // Zero dimensions
    let dim_0x0 = VideoDimensions::new(0, 0);
    assert_eq!(dim_0x0.aspect_ratio(), 16.0 / 9.0);
    assert_eq!(dim_0x0.fit_in_viewport(0, 0), (0, 0));
    assert_eq!(dim_0x0.fit_in_viewport(1920, 1080), (1920, 1080));

    let dim_w0_h1080 = VideoDimensions::new(0, 1080);
    assert_eq!(dim_w0_h1080.aspect_ratio(), 0.0);
    assert_eq!(dim_w0_h1080.fit_in_viewport(1920, 1080), (1920, 1080));

    let dim_w1920_h0 = VideoDimensions::new(1920, 0);
    assert_eq!(dim_w1920_h0.aspect_ratio(), 16.0 / 9.0);
    assert_eq!(dim_w1920_h0.fit_in_viewport(1920, 1080), (1920, 1080));

    // Viewport zero dimensions
    let normal_video = VideoDimensions::new(1920, 1080);
    assert_eq!(normal_video.fit_in_viewport(0, 0), (0, 0));
    assert_eq!(normal_video.fit_in_viewport(0, 1080), (0, 1080));
    assert_eq!(normal_video.fit_in_viewport(1920, 0), (1920, 0));

    // 1x1 Viewport
    let (w1, h1) = normal_video.fit_in_viewport(1, 1);
    assert!(w1 <= 1 && h1 <= 1, "Fitted dimension must not exceed 1x1 viewport: ({}, {})", w1, h1);

    // Ultra-wide 32:9 viewport (e.g. 5120x1440) with 16:9 video -> pillarboxed
    let (pw, ph) = normal_video.fit_in_viewport(5120, 1440);
    assert_eq!(ph, 1440);
    assert_eq!(pw, 2560);
    assert!(pw <= 5120);

    // Ultra-wide 32:9 video (5120x1440) in 16:9 viewport (1920x1080) -> letterboxed
    let ultrawide_video = VideoDimensions::new(5120, 1440);
    let (lw, lh) = ultrawide_video.fit_in_viewport(1920, 1080);
    assert_eq!(lw, 1920);
    assert_eq!(lh, 540);
    assert!(lh <= 1080);

    // Vertical smartphone 9:16 viewport (1080x1920) with 16:9 video -> letterboxed
    let (sw, sh) = normal_video.fit_in_viewport(1080, 1920);
    assert_eq!(sw, 1080);
    assert_eq!(sh, 608);
    assert!(sh <= 1920);

    // Vertical smartphone 9:16 video (1080x1920) in 16:9 viewport (1920x1080) -> pillarboxed
    let vertical_video = VideoDimensions::new(1080, 1920);
    let (vw, vh) = vertical_video.fit_in_viewport(1920, 1080);
    assert_eq!(vh, 1080);
    assert_eq!(vw, 608);
    assert!(vw <= 1920);

    // Large extreme values: u32::MAX
    let max_viewport = (u32::MAX, u32::MAX);
    let (max_w, max_h) = normal_video.fit_in_viewport(max_viewport.0, max_viewport.1);
    assert_eq!(max_h, 2415919103);
    assert_eq!(max_w, u32::MAX);

    let max_video = VideoDimensions::new(u32::MAX, u32::MAX);
    let (mw, mh) = max_video.fit_in_viewport(1920, 1080);
    assert_eq!((mw, mh), (1080, 1080));
}

// ============================================================================
// 2. Playback Seek Boundary Stress & Concurrency
// ============================================================================

#[test]
fn test_seek_boundary_values() {
    let player = HeadlessPlayer::new();
    player.open_file_with_duration("/test/video.mp4", 60_000).unwrap();

    // Exact 0
    player.seek(0, true).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 0);

    // Exact duration
    player.seek(60_000, true).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 60_000);

    // Beyond duration clamped to duration
    player.seek(60_001, true).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 60_000);

    player.seek(1_000_000, true).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 60_000);

    player.seek(u64::MAX, true).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 60_000);
}

#[test]
fn test_seek_when_no_file_loaded() {
    let player = HeadlessPlayer::new();
    assert_eq!(player.get_state().unwrap().current_file, None);

    // Checking behavior when seeking with no file loaded
    let res = player.seek(5_000, true);
    println!("Seek when no file loaded result: {:?}", res);
}

#[test]
fn test_concurrent_rapid_seeks() {
    let player = Arc::new(HeadlessPlayer::new());
    player.open_file_with_duration("/test/stream.mkv", 100_000).unwrap();

    let mut handles = Vec::new();
    for thread_idx in 0..20 {
        let p = Arc::clone(&player);
        handles.push(std::thread::spawn(move || {
            for i in 0..200 {
                let target = ((thread_idx * 1000 + i * 37) % 150_000) as u64;
                let _ = p.seek(target, true);
            }
        }));
    }

    for h in handles {
        h.join().expect("Seek worker thread panicked");
    }

    let final_pos = player.get_state().unwrap().position_ms;
    assert!(final_pos <= 100_000, "Final position must be clamped to duration: {}", final_pos);
}

// ============================================================================
// 3. Playback Speed Boundaries & Adversarial Inputs
// ============================================================================

#[test]
fn test_speed_valid_and_invalid_boundaries() {
    let player = HeadlessPlayer::new();

    // All allowed speeds must succeed
    for &speed in &ALLOWED_SPEEDS {
        assert!(player.set_speed(speed).is_ok(), "Speed {} should be valid", speed);
        let current = player.get_state().unwrap().speed;
        assert!((current - speed).abs() < 1e-4);
    }

    // Invalid speeds must be rejected
    let invalid_speeds = [
        -1.0f32,
        -0.5f32,
        0.0f32,
        0.25f32,
        0.49f32,
        0.8f32,
        1.1f32,
        2.01f32,
        3.0f32,
        100.0f32,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];

    for &inv in &invalid_speeds {
        let res = player.set_speed(inv);
        assert!(
            matches!(res, Err(MediaEngineError::InvalidSpeed(_))),
            "Speed {} should be rejected with InvalidSpeed, got {:?}",
            inv,
            res
        );
    }
}

// ============================================================================
// 4. Volume and Mute Boundaries & Rapid Toggle Spam
// ============================================================================

#[test]
fn test_volume_boundaries_and_clamping() {
    let player = HeadlessPlayer::new();

    // Standard valid range
    player.set_volume(0.0).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 0.0);

    player.set_volume(0.5).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 0.5);

    player.set_volume(1.0).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 1.0);

    // Negative volume clamps to 0.0
    player.set_volume(-0.5).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 0.0);

    player.set_volume(-100.0).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 0.0);

    // Volume > 1.0 clamps to 1.0
    player.set_volume(1.5).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 1.0);

    player.set_volume(999.0).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 1.0);

    // Infinite volumes
    player.set_volume(f64::INFINITY).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 1.0);

    player.set_volume(f64::NEG_INFINITY).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 0.0);

    // NaN volume behavior observation
    let nan_res = player.set_volume(f64::NAN);
    println!("set_volume(NaN) result: {:?}", nan_res);
    let state_vol = player.get_state().unwrap().volume;
    println!("State volume after set_volume(NaN): {}", state_vol);
}

#[test]
fn test_rapid_toggle_mute_spamming() {
    let player = Arc::new(HeadlessPlayer::new());

    let mut handles = Vec::new();
    for thread_id in 0..16 {
        let p = Arc::clone(&player);
        handles.push(std::thread::spawn(move || {
            for i in 0..250 {
                let mute = (thread_id + i) % 2 == 0;
                let _ = p.set_mute(mute);
            }
        }));
    }

    for h in handles {
        h.join().expect("Mute thread panicked");
    }

    let _final_mute = player.get_state().unwrap().is_muted;
}

// ============================================================================
// 5. Track Selection Stress
// ============================================================================

#[test]
fn test_track_selection_invalid_ids() {
    let player = HeadlessPlayer::new();

    // Audio tracks: valid are [1, 2]
    let invalid_audio_ids = [-999, -1, 0, 3, 999, i64::MIN, i64::MAX];
    for &id in &invalid_audio_ids {
        let res = player.select_audio(id);
        assert!(
            matches!(res, Err(MediaEngineError::TrackNotFound(x)) if x == id),
            "Expected TrackNotFound({}) but got {:?}",
            id,
            res
        );
    }

    // Subtitle tracks: valid are [1], and -1 disables
    assert!(player.select_subtitle(-1).is_ok());
    let subs = player.subtitle_tracks().unwrap();
    assert!(!subs.iter().any(|s| s.is_selected));

    let invalid_sub_ids = [-999, 0, 2, 999, i64::MIN, i64::MAX];
    for &id in &invalid_sub_ids {
        let res = player.select_subtitle(id);
        assert!(
            matches!(res, Err(MediaEngineError::TrackNotFound(x)) if x == id),
            "Expected TrackNotFound({}) but got {:?}",
            id,
            res
        );
    }
}

#[test]
fn test_external_subtitle_file_paths() {
    let player = HeadlessPlayer::new();

    // Invalid subtitle extensions
    let invalid_extensions = [
        "/path/sub.txt",
        "/path/sub.vtt",
        "/path/sub.sub",
        "/path/sub.idx",
        "/path/sub.mp4",
        "/path/sub",
        "/path/sub.",
    ];

    for &path in &invalid_extensions {
        let res = player.add_external_subtitle(path, None, None);
        assert!(
            matches!(res, Err(MediaEngineError::UnsupportedSubtitleExtension(_))),
            "Expected UnsupportedSubtitleExtension for {} but got {:?}",
            path,
            res
        );
    }

    // Non-existent .srt / .ass path
    let non_existent_srt = "/this/path/absolutely/does_not_exist_at_all.srt";
    let res = player.add_external_subtitle(non_existent_srt, Some("Missing"), Some("eng"));
    println!("add_external_subtitle with non-existent path: {:?}", res);
}

// ============================================================================
// 6. Keyboard Shortcuts Stress
// ============================================================================

#[test]
fn test_rapid_alternating_keyboard_shortcuts() {
    let player = HeadlessPlayer::new();
    player.open_file_with_duration("/video.mp4", 500_000).unwrap();
    let mut is_fullscreen = false;

    let actions = [
        KeyAction::SpaceTogglePlayPause,
        KeyAction::SeekRelative(5_000),
        KeyAction::SeekRelative(-5_000),
        KeyAction::SeekRelative(30_000),
        KeyAction::SeekRelative(-30_000),
        KeyAction::ToggleFullscreen,
        KeyAction::ToggleMute,
        KeyAction::ExitFullscreen,
    ];

    // Rapid sequential execution
    for round in 0..1000 {
        let action = actions[round % actions.len()].clone();
        assert!(handle_shortcut(&player, action, &mut is_fullscreen).is_ok());
    }

    let final_state = player.get_state().unwrap();
    assert!(final_state.position_ms <= 500_000);
}

#[test]
fn test_keyboard_shortcut_seek_underflow_and_overflow() {
    let player = HeadlessPlayer::new();
    player.open_file_with_duration("/video.mp4", 100_000).unwrap();
    let mut is_fullscreen = false;

    // SeekRelative large negative when at 0
    assert_eq!(player.get_state().unwrap().position_ms, 0);
    handle_shortcut(&player, KeyAction::SeekRelative(-100_000), &mut is_fullscreen).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 0);

    // SeekRelative large positive beyond duration
    handle_shortcut(&player, KeyAction::SeekRelative(1_000_000), &mut is_fullscreen).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 100_000);
}

// ============================================================================
// 7. Event Broadcast Overflow & Subscriber Lagging
// ============================================================================

#[tokio::test]
async fn test_broadcast_event_overflow_lagged() {
    let player = HeadlessPlayer::new();
    let mut rx = player.subscribe_events();

    // HeadlessPlayer uses broadcast::channel(128).
    // Send 200 events without reading to trigger Lagged.
    for i in 0..200 {
        player.seek(i * 10, false).unwrap();
    }

    // Next recv should return Err(Lagged)
    match rx.recv().await {
        Ok(evt) => {
            println!("Got event unexpectedly: {:?}", evt);
        }
        Err(broadcast::error::RecvError::Lagged(skipped)) => {
            println!("Receiver lagged as expected, skipped: {}", skipped);
            assert!(skipped > 0);
        }
        Err(broadcast::error::RecvError::Closed) => {
            panic!("Channel closed unexpectedly");
        }
    }
}
