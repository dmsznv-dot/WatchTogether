use media_engine::dimensions::VideoDimensions;
use media_engine::error::MediaEngineError;
use media_engine::events::PlayerEvent;
use media_engine::headless::HeadlessPlayer;
use media_engine::mpv::MpvPlayer;
use media_engine::shortcuts::{handle_shortcut, KeyAction};
use media_engine::track::TrackType;
use media_engine::traits::MediaPlayer;
use media_engine::create_player;

#[test]
fn test_headless_player_initial_state() {
    let player = HeadlessPlayer::new();
    let state = player.get_state().unwrap();

    assert_eq!(state.current_file, None);
    assert!(!state.is_playing);
    assert_eq!(state.position_ms, 0);
    assert_eq!(state.duration_ms, 0);
    assert_eq!(state.speed, 1.0);
    assert_eq!(state.volume, 1.0);
    assert!(!state.is_muted);

    let playback_state = player.playback_state().unwrap();
    assert_eq!(playback_state.status, shared_protocol::models::PlaybackStatus::Paused);
    assert_eq!(playback_state.position_ms, 0);
}

#[test]
fn test_headless_player_open_and_playback_lifecycle() {
    let player = HeadlessPlayer::new();
    player.open_file_with_duration("/path/to/test.mp4", 120_000).unwrap();

    let state = player.get_state().unwrap();
    assert_eq!(state.current_file.as_deref(), Some("/path/to/test.mp4"));
    assert_eq!(state.duration_ms, 120_000);
    assert_eq!(state.position_ms, 0);
    assert!(!state.is_playing);

    // Play
    player.play().unwrap();
    assert!(player.get_state().unwrap().is_playing);

    // Pause
    player.pause().unwrap();
    assert!(!player.get_state().unwrap().is_playing);

    // Seek
    player.seek(45_000, true).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 45_000);

    // Seek beyond duration clamps
    player.seek(200_000, true).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 120_000);
}

#[test]
fn test_headless_player_speed_validation() {
    let player = HeadlessPlayer::new();

    let valid_speeds = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
    for s in valid_speeds {
        assert!(player.set_speed(s).is_ok());
        assert_eq!(player.get_state().unwrap().speed, s);
    }

    let invalid_speeds = [-1.0, 0.0, 0.1, 0.4, 2.1, 4.0, 10.0];
    for s in invalid_speeds {
        assert!(matches!(
            player.set_speed(s),
            Err(MediaEngineError::InvalidSpeed(_))
        ));
    }
}

#[test]
fn test_headless_player_volume_and_mute() {
    let player = HeadlessPlayer::new();

    player.set_volume(0.5).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 0.5);

    // Clamping
    player.set_volume(1.5).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 1.0);

    player.set_volume(-0.5).unwrap();
    assert_eq!(player.get_state().unwrap().volume, 0.0);

    // Mute toggle
    player.set_volume(0.75).unwrap();
    player.set_mute(true).unwrap();
    assert!(player.get_state().unwrap().is_muted);
    assert_eq!(player.get_state().unwrap().volume, 0.75);

    player.set_mute(false).unwrap();
    assert!(!player.get_state().unwrap().is_muted);
    assert_eq!(player.get_state().unwrap().volume, 0.75);
}

#[test]
fn test_headless_player_audio_tracks() {
    let player = HeadlessPlayer::new();
    let tracks = player.audio_tracks().unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].id, 1);
    assert_eq!(tracks[0].track_type, TrackType::Audio);
    assert!(tracks[0].is_selected);
    assert_eq!(tracks[1].id, 2);
    assert!(!tracks[1].is_selected);

    // Select secondary track
    player.select_audio(2).unwrap();
    let updated = player.audio_tracks().unwrap();
    assert!(!updated[0].is_selected);
    assert!(updated[1].is_selected);

    // Non-existent track returns error
    assert!(matches!(
        player.select_audio(999),
        Err(MediaEngineError::TrackNotFound(999))
    ));
}

#[test]
fn test_headless_player_subtitle_tracks() {
    let player = HeadlessPlayer::new();
    let subs = player.subtitle_tracks().unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].id, 1);
    assert_eq!(subs[0].track_type, TrackType::Subtitle);
    assert!(!subs[0].is_selected);

    // Select embedded subtitle
    player.select_subtitle(1).unwrap();
    assert!(player.subtitle_tracks().unwrap()[0].is_selected);

    // Disable subtitles with -1
    player.select_subtitle(-1).unwrap();
    assert!(!player.subtitle_tracks().unwrap()[0].is_selected);

    // Load external .srt
    let srt_id = player
        .add_external_subtitle("/subs/en.srt", Some("English External"), Some("eng"))
        .unwrap();
    assert_eq!(srt_id, 2);
    let subs_after_srt = player.subtitle_tracks().unwrap();
    assert_eq!(subs_after_srt.len(), 2);
    assert!(subs_after_srt[1].is_selected);
    assert!(subs_after_srt[1].is_external);

    // Load external .ass
    let ass_id = player
        .add_external_subtitle("/subs/jp.ass", Some("Japanese ASS"), Some("jpn"))
        .unwrap();
    assert_eq!(ass_id, 3);

    // Unsupported extension rejected
    assert!(matches!(
        player.add_external_subtitle("/subs/test.txt", None, None),
        Err(MediaEngineError::UnsupportedSubtitleExtension(_))
    ));
    assert!(matches!(
        player.add_external_subtitle("/subs/test.vtt", None, None),
        Err(MediaEngineError::UnsupportedSubtitleExtension(_))
    ));
}

#[test]
fn test_video_dimensions_aspect_ratio_and_fitting() {
    // 16:9
    let v16_9 = VideoDimensions::new(1920, 1080);
    assert!((v16_9.aspect_ratio() - (16.0 / 9.0)).abs() < 1e-4);

    // 1000x1000 viewport -> letterboxed (1000, 563)
    let (w, h) = v16_9.fit_in_viewport(1000, 1000);
    assert_eq!(w, 1000);
    assert_eq!(h, 563);

    // 21:9 ultrawide in 3000x1000 viewport -> pillarboxed (2370, 1000)
    let v21_9 = VideoDimensions::new(2560, 1080);
    let (w, h) = v21_9.fit_in_viewport(3000, 1000);
    assert_eq!(w, 2370);
    assert_eq!(h, 1000);

    // 1:1 square in 1920x1080 -> (1080, 1080)
    let v1_1 = VideoDimensions::new(1080, 1080);
    let (w, h) = v1_1.fit_in_viewport(1920, 1080);
    assert_eq!(w, 1080);
    assert_eq!(h, 1080);

    // 9:16 portrait in 1920x1080 -> (608, 1080)
    let v9_16 = VideoDimensions::new(1080, 1920);
    let (w, h) = v9_16.fit_in_viewport(1920, 1080);
    assert_eq!(w, 608);
    assert_eq!(h, 1080);

    // Fallback on zero dimension
    let v0 = VideoDimensions::new(0, 0);
    let (w, h) = v0.fit_in_viewport(800, 600);
    assert_eq!(w, 800);
    assert_eq!(h, 600);
}

#[test]
fn test_keyboard_shortcuts() {
    let player = HeadlessPlayer::new();
    player.open_file_with_duration("/vid.mp4", 100_000).unwrap();
    let mut is_fullscreen = false;

    // Space toggles play / pause
    assert!(!player.get_state().unwrap().is_playing);
    handle_shortcut(&player, KeyAction::SpaceTogglePlayPause, &mut is_fullscreen).unwrap();
    assert!(player.get_state().unwrap().is_playing);
    handle_shortcut(&player, KeyAction::SpaceTogglePlayPause, &mut is_fullscreen).unwrap();
    assert!(!player.get_state().unwrap().is_playing);

    // SeekRelative +5000
    handle_shortcut(&player, KeyAction::SeekRelative(5_000), &mut is_fullscreen).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 5_000);

    // SeekRelative -5000
    handle_shortcut(&player, KeyAction::SeekRelative(-5_000), &mut is_fullscreen).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 0);

    // SeekRelative -5000 at 0 clamps to 0
    handle_shortcut(&player, KeyAction::SeekRelative(-5_000), &mut is_fullscreen).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 0);

    // SeekRelative +30000
    handle_shortcut(&player, KeyAction::SeekRelative(30_000), &mut is_fullscreen).unwrap();
    assert_eq!(player.get_state().unwrap().position_ms, 30_000);

    // Fullscreen toggle & exit
    assert!(!is_fullscreen);
    handle_shortcut(&player, KeyAction::ToggleFullscreen, &mut is_fullscreen).unwrap();
    assert!(is_fullscreen);
    handle_shortcut(&player, KeyAction::ExitFullscreen, &mut is_fullscreen).unwrap();
    assert!(!is_fullscreen);

    // Toggle mute
    assert!(!player.get_state().unwrap().is_muted);
    handle_shortcut(&player, KeyAction::ToggleMute, &mut is_fullscreen).unwrap();
    assert!(player.get_state().unwrap().is_muted);
    handle_shortcut(&player, KeyAction::ToggleMute, &mut is_fullscreen).unwrap();
    assert!(!player.get_state().unwrap().is_muted);
}

#[tokio::test]
async fn test_player_events_broadcast() {
    let player = HeadlessPlayer::new();
    let mut rx = player.subscribe_events();

    player.play().unwrap();
    let event = rx.recv().await.unwrap();
    match event {
        PlayerEvent::StateChange { is_playing, .. } => assert!(is_playing),
        _ => panic!("Expected StateChange event, got {:?}", event),
    }

    player.seek(25_000, true).unwrap();
    let event = rx.recv().await.unwrap();
    match event {
        PlayerEvent::TimePos { position_ms, .. } => assert_eq!(position_ms, 25_000),
        _ => panic!("Expected TimePos event, got {:?}", event),
    }
}

#[test]
fn test_mpv_player_not_found_on_host_or_fallback() {
    // When libmpv is not present on the host system, MpvPlayer::new() returns LibMpvNotFound
    let mpv_res = MpvPlayer::new();
    match mpv_res {
        Ok(player) => {
            // If host happens to have libmpv installed, it should work
            let state = player.get_state().unwrap();
            assert_eq!(state.position_ms, 0);
        }
        Err(e) => {
            assert!(matches!(
                e,
                MediaEngineError::LibMpvNotFound(_) | MediaEngineError::LibMpvInitFailed(_)
            ));
        }
    }

    // create_player factory always succeeds by falling back to HeadlessPlayer
    let fallback_player = create_player();
    assert!(fallback_player.play().is_ok());
    assert!(fallback_player.pause().is_ok());
}
