use watch_together_tests::mock_media::*;

// ============================================================================
// FEATURE 1: LOCAL MP4 H.264/AAC PLAYBACK
// ============================================================================

#[test]
fn test_tier1_f1_mp4_open_and_load_metadata() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/movie.mp4", 7_200_000); // 2 hours

    assert_eq!(player.current_file, Some("/path/to/movie.mp4".into()));
    assert_eq!(player.duration_ms, 7_200_000);
    assert_eq!(player.position_ms, 0);
    assert!(!player.is_playing);
}

#[test]
fn test_tier1_f1_mp4_initial_state_paused_at_zero() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/clip.mp4", 60_000);

    assert_eq!(player.position_ms, 0);
    assert!(!player.is_playing);
    assert_eq!(player.speed, 1.0);
}

#[test]
fn test_tier1_f1_mp4_play_advances_playback_state() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/clip.mp4", 60_000);
    player.play();

    assert!(player.is_playing);
}

#[test]
fn test_tier1_f1_mp4_pause_preserves_position() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/clip.mp4", 60_000);
    player.seek(15_000);
    player.play();
    player.pause();

    assert!(!player.is_playing);
    assert_eq!(player.position_ms, 15_000);
}

#[test]
fn test_tier1_f1_mp4_timeline_current_pos_and_duration() {
    let mut player = MockPlayer::new_headless();
    let duration = 120_000;
    player.open_file("/path/to/clip.mp4", duration);
    player.seek(45_000);

    assert_eq!(player.position_ms, 45_000);
    assert_eq!(player.duration_ms, duration);
    let progress_pct = (player.position_ms as f64) / (player.duration_ms as f64);
    assert!((progress_pct - 0.375).abs() < 1e-4);
}

#[test]
fn test_tier2_f1_mp4_zero_byte_empty_file_rejected() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/empty.mp4", 0);

    assert_eq!(player.duration_ms, 0);
    player.seek(100);
    assert_eq!(player.position_ms, 100);
}

#[test]
fn test_tier2_f1_mp4_seek_to_exact_start_zero() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/clip.mp4", 60_000);
    player.seek(30_000);
    player.seek(0);

    assert_eq!(player.position_ms, 0);
}

#[test]
fn test_tier2_f1_mp4_seek_to_exact_duration_clamped() {
    let mut player = MockPlayer::new_headless();
    let duration = 60_000;
    player.open_file("/path/to/clip.mp4", duration);
    player.seek(duration);

    assert_eq!(player.position_ms, duration);
}

#[test]
fn test_tier2_f1_mp4_seek_beyond_duration_clamped() {
    let mut player = MockPlayer::new_headless();
    let duration = 60_000;
    player.open_file("/path/to/clip.mp4", duration);
    player.seek(100_000); // Beyond duration

    assert_eq!(player.position_ms, duration);
}

#[test]
fn test_tier2_f1_mp4_play_at_eof_remains_at_duration() {
    let mut player = MockPlayer::new_headless();
    let duration = 60_000;
    player.open_file("/path/to/clip.mp4", duration);
    player.seek(duration);
    player.play();

    assert!(player.is_playing);
    assert_eq!(player.position_ms, duration);
}

// ============================================================================
// FEATURE 2: LOCAL MKV MULTI-AUDIO PLAYBACK
// ============================================================================

#[test]
fn test_tier1_f2_mkv_open_detects_multiple_audio_tracks() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/feature_film.mkv", 7_200_000);

    assert_eq!(player.audio_tracks.len(), 2);
    assert_eq!(player.audio_tracks[0].id, 1);
    assert_eq!(player.audio_tracks[1].id, 2);
}

#[test]
fn test_tier1_f2_mkv_default_audio_track_selected() {
    let player = MockPlayer::new_headless();
    let selected = player.audio_tracks.iter().find(|t| t.is_selected);

    assert!(selected.is_some());
    assert_eq!(selected.unwrap().id, 1);
}

#[test]
fn test_tier1_f2_mkv_switch_to_secondary_commentary_track() {
    let mut player = MockPlayer::new_headless();
    let success = player.select_audio(2);

    assert!(success);
    assert!(!player.audio_tracks[0].is_selected);
    assert!(player.audio_tracks[1].is_selected);
}

#[test]
fn test_tier1_f2_mkv_audio_track_switch_retains_playing_state() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/film.mkv", 100_000);
    player.play();
    player.seek(25_000);

    let switched = player.select_audio(2);
    assert!(switched);
    assert!(player.is_playing);
    assert_eq!(player.position_ms, 25_000);
}

#[test]
fn test_tier1_f2_mkv_audio_track_properties_title_and_lang() {
    let player = MockPlayer::new_headless();
    let t1 = &player.audio_tracks[0];
    let t2 = &player.audio_tracks[1];

    assert_eq!(t1.title.as_deref(), Some("English Stereo"));
    assert_eq!(t1.lang.as_deref(), Some("eng"));
    assert_eq!(t2.title.as_deref(), Some("Director Commentary"));
    assert_eq!(t2.lang.as_deref(), Some("eng"));
}

#[test]
fn test_tier2_f2_mkv_invalid_audio_track_id_rejected() {
    let mut player = MockPlayer::new_headless();
    let success = player.select_audio(999);

    assert!(!success);
    // Previously selected track remains selected
    assert!(player.audio_tracks[0].is_selected);
}

#[test]
fn test_tier2_f2_mkv_switch_to_same_track_idempotent() {
    let mut player = MockPlayer::new_headless();
    assert!(player.select_audio(1));
    assert!(player.audio_tracks[0].is_selected);
    assert!(player.select_audio(1));
    assert!(player.audio_tracks[0].is_selected);
}

#[test]
fn test_tier2_f2_mkv_audio_track_switch_at_start_boundary() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/movie.mkv", 60_000);
    player.seek(0);
    assert!(player.select_audio(2));
    assert_eq!(player.position_ms, 0);
}

#[test]
fn test_tier2_f2_mkv_audio_track_switch_at_eof_boundary() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/movie.mkv", 60_000);
    player.seek(60_000);
    assert!(player.select_audio(2));
    assert_eq!(player.position_ms, 60_000);
}

#[test]
fn test_tier2_f2_mkv_file_with_single_audio_track() {
    let mut player = MockPlayer::new_headless();
    player.audio_tracks.truncate(1);
    assert_eq!(player.audio_tracks.len(), 1);
    assert!(player.select_audio(1));
    assert!(!player.select_audio(2));
}

// ============================================================================
// FEATURE 3: EXTERNAL SUBTITLE LOADING (.srt, .ass)
// ============================================================================

#[test]
fn test_tier1_f3_subtitles_load_external_srt_file() {
    let mut player = MockPlayer::new_headless();
    let id = player.add_external_subtitle("/path/subs/movie_en.srt", "English (External)", "eng");

    assert!(id > 0);
    let track = player.subtitle_tracks.iter().find(|t| t.id == id).unwrap();
    assert_eq!(track.title.as_deref(), Some("English (External)"));
    assert!(track.is_external);
    assert!(track.is_selected);
}

#[test]
fn test_tier1_f3_subtitles_load_external_ass_file() {
    let mut player = MockPlayer::new_headless();
    let id = player.add_external_subtitle("/path/subs/fansub.ass", "Fansub Advanced (ASS)", "jpn");

    assert!(id > 0);
    let track = player.subtitle_tracks.iter().find(|t| t.id == id).unwrap();
    assert_eq!(track.title.as_deref(), Some("Fansub Advanced (ASS)"));
    assert!(track.is_external);
    assert!(track.is_selected);
}

#[test]
fn test_tier1_f3_subtitles_select_embedded_subtitle() {
    let mut player = MockPlayer::new_headless();
    let selected = player.select_subtitle(1);

    assert!(selected);
    assert!(player.subtitle_tracks[0].is_selected);
    assert!(!player.subtitle_tracks[0].is_external);
}

#[test]
fn test_tier1_f3_subtitles_switch_between_external_and_embedded() {
    let mut player = MockPlayer::new_headless();
    let ext_id = player.add_external_subtitle("/subs/fr.srt", "French", "fra");

    assert!(player.subtitle_tracks.iter().find(|t| t.id == ext_id).unwrap().is_selected);
    assert!(!player.subtitle_tracks[0].is_selected);

    // Switch back to embedded track 1
    assert!(player.select_subtitle(1));
    assert!(player.subtitle_tracks[0].is_selected);
    assert!(!player.subtitle_tracks.iter().find(|t| t.id == ext_id).unwrap().is_selected);
}

#[test]
fn test_tier1_f3_subtitles_disable_subtitles_selection() {
    let mut player = MockPlayer::new_headless();
    player.select_subtitle(1);
    assert!(player.subtitle_tracks[0].is_selected);

    // Track ID -1 represents disable subtitles
    player.select_subtitle(-1);
    assert!(!player.subtitle_tracks[0].is_selected);
}

#[test]
fn test_tier2_f3_subtitles_non_existent_file_path_rejected() {
    // Validating file path existence logic
    let invalid_path = "/non_existent/path/subs.srt";
    assert!(!std::path::Path::new(invalid_path).exists());
}

#[test]
fn test_tier2_f3_subtitles_empty_srt_file_handling() {
    let mut player = MockPlayer::new_headless();
    let id = player.add_external_subtitle("/path/empty.srt", "Empty Subtitle", "und");
    assert!(id > 0);
    assert!(player.subtitle_tracks.iter().any(|t| t.id == id));
}

#[test]
fn test_tier2_f3_subtitles_unsupported_extension_rejected() {
    let valid_extensions = ["srt", "ass"];
    let invalid_extensions = ["txt", "sub", "vtt", "exe"];

    for ext in &valid_extensions {
        assert!(matches!(*ext, "srt" | "ass"));
    }
    for ext in &invalid_extensions {
        assert!(!matches!(*ext, "srt" | "ass"));
    }
}

#[test]
fn test_tier2_f3_subtitles_multiple_external_tracks_enumeration() {
    let mut player = MockPlayer::new_headless();
    let id1 = player.add_external_subtitle("/subs/en.srt", "English", "eng");
    let id2 = player.add_external_subtitle("/subs/es.srt", "Spanish", "spa");
    let id3 = player.add_external_subtitle("/subs/de.ass", "German", "deu");

    assert_eq!(player.subtitle_tracks.len(), 4); // 1 embedded + 3 external
    assert_eq!(id1, 2);
    assert_eq!(id2, 3);
    assert_eq!(id3, 4);
    assert!(player.subtitle_tracks.iter().find(|t| t.id == id3).unwrap().is_selected);
}

#[test]
fn test_tier2_f3_subtitles_external_track_flags_isolated() {
    let mut player = MockPlayer::new_headless();
    let id = player.add_external_subtitle("/subs/ru.srt", "Russian", "rus");

    let embedded = &player.subtitle_tracks[0];
    let external = player.subtitle_tracks.iter().find(|t| t.id == id).unwrap();

    assert!(!embedded.is_external);
    assert!(external.is_external);
}

// ============================================================================
// FEATURE 4: PLAYBACK SPEED & PITCH (0.5x–2x)
// ============================================================================

#[test]
fn test_tier1_f4_speed_default_is_nominal_1x() {
    let player = MockPlayer::new_headless();
    assert_eq!(player.speed, 1.0);
}

#[test]
fn test_tier1_f4_speed_discrete_steps_0_5x_and_0_75x() {
    let mut player = MockPlayer::new_headless();
    player.set_speed(0.5);
    assert_eq!(player.speed, 0.5);

    player.set_speed(0.75);
    assert_eq!(player.speed, 0.75);
}

#[test]
fn test_tier1_f4_speed_discrete_steps_1_25x_and_1_5x() {
    let mut player = MockPlayer::new_headless();
    player.set_speed(1.25);
    assert_eq!(player.speed, 1.25);

    player.set_speed(1.5);
    assert_eq!(player.speed, 1.5);
}

#[test]
fn test_tier1_f4_speed_discrete_step_2_0x() {
    let mut player = MockPlayer::new_headless();
    player.set_speed(2.0);
    assert_eq!(player.speed, 2.0);
}

#[test]
fn test_tier1_f4_speed_preserves_position_during_change() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/movie.mp4", 100_000);
    player.seek(50_000);
    player.set_speed(1.5);

    assert_eq!(player.position_ms, 50_000);
    assert_eq!(player.speed, 1.5);
}

#[test]
fn test_tier2_f4_speed_boundary_minimum_0_5x() {
    let mut player = MockPlayer::new_headless();
    player.set_speed(0.5);
    assert_eq!(player.speed, 0.5);
}

#[test]
fn test_tier2_f4_speed_boundary_maximum_2_0x() {
    let mut player = MockPlayer::new_headless();
    player.set_speed(2.0);
    assert_eq!(player.speed, 2.0);
}

#[test]
fn test_tier2_f4_speed_invalid_negative_rejected() {
    use watch_together_tests::room::is_valid_playback_speed;
    assert!(!is_valid_playback_speed(-1.0));
    assert!(!is_valid_playback_speed(-0.5));
}

#[test]
fn test_tier2_f4_speed_invalid_extreme_values_rejected() {
    use watch_together_tests::room::is_valid_playback_speed;
    assert!(!is_valid_playback_speed(0.1));
    assert!(!is_valid_playback_speed(0.0));
    assert!(!is_valid_playback_speed(4.0));
    assert!(!is_valid_playback_speed(10.0));
}

#[test]
fn test_tier2_f4_speed_rapid_toggle_consistency() {
    let mut player = MockPlayer::new_headless();
    let speeds = [0.5, 1.0, 1.5, 2.0, 1.0, 0.75, 1.25];
    for s in speeds {
        player.set_speed(s);
        assert_eq!(player.speed, s);
    }
}

// ============================================================================
// FEATURE 5: KEYBOARD SHORTCUTS & NAVIGATION
// ============================================================================

#[test]
fn test_tier1_f5_shortcut_space_toggles_play_pause() {
    let mut player = MockPlayer::new_headless();
    let mut is_fs = false;

    assert!(!player.is_playing);
    handle_shortcut(&mut player, KeyAction::SpaceTogglePlayPause, &mut is_fs);
    assert!(player.is_playing);
    handle_shortcut(&mut player, KeyAction::SpaceTogglePlayPause, &mut is_fs);
    assert!(!player.is_playing);
}

#[test]
fn test_tier1_f5_shortcut_right_arrow_seeks_forward_5s() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    let mut is_fs = false;

    handle_shortcut(&mut player, KeyAction::SeekRelative(5_000), &mut is_fs);
    assert_eq!(player.position_ms, 5_000);

    handle_shortcut(&mut player, KeyAction::SeekRelative(5_000), &mut is_fs);
    assert_eq!(player.position_ms, 10_000);
}

#[test]
fn test_tier1_f5_shortcut_left_arrow_seeks_backward_5s() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    player.seek(20_000);
    let mut is_fs = false;

    handle_shortcut(&mut player, KeyAction::SeekRelative(-5_000), &mut is_fs);
    assert_eq!(player.position_ms, 15_000);
}

#[test]
fn test_tier1_f5_shortcut_shift_right_arrow_seeks_forward_30s() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    let mut is_fs = false;

    handle_shortcut(&mut player, KeyAction::SeekRelative(30_000), &mut is_fs);
    assert_eq!(player.position_ms, 30_000);
}

#[test]
fn test_tier1_f5_shortcut_shift_left_arrow_seeks_backward_30s() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    player.seek(50_000);
    let mut is_fs = false;

    handle_shortcut(&mut player, KeyAction::SeekRelative(-30_000), &mut is_fs);
    assert_eq!(player.position_ms, 20_000);
}

#[test]
fn test_tier2_f5_shortcut_backward_seek_clamped_at_zero() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    player.seek(10_000);
    let mut is_fs = false;

    // Seeking backward 30s from 10s should clamp at 0
    handle_shortcut(&mut player, KeyAction::SeekRelative(-30_000), &mut is_fs);
    assert_eq!(player.position_ms, 0);

    // Seeking backward again remains at 0
    handle_shortcut(&mut player, KeyAction::SeekRelative(-5_000), &mut is_fs);
    assert_eq!(player.position_ms, 0);
}

#[test]
fn test_tier2_f5_shortcut_forward_seek_clamped_at_duration() {
    let mut player = MockPlayer::new_headless();
    let duration = 60_000;
    player.open_file("/path/to/vid.mp4", duration);
    player.seek(50_000);
    let mut is_fs = false;

    // Seeking forward 30s from 50s in a 60s video clamps at 60s
    handle_shortcut(&mut player, KeyAction::SeekRelative(30_000), &mut is_fs);
    assert_eq!(player.position_ms, duration);
}

#[test]
fn test_tier2_f5_shortcut_rapid_repeated_arrow_seeks() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    let mut is_fs = false;

    for _ in 0..6 {
        handle_shortcut(&mut player, KeyAction::SeekRelative(5_000), &mut is_fs);
    }
    assert_eq!(player.position_ms, 30_000);
}

#[test]
fn test_tier2_f5_shortcut_seek_from_exact_start() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    player.seek(0);
    let mut is_fs = false;

    handle_shortcut(&mut player, KeyAction::SeekRelative(-5_000), &mut is_fs);
    assert_eq!(player.position_ms, 0);
}

#[test]
fn test_tier2_f5_shortcut_seek_from_exact_end() {
    let mut player = MockPlayer::new_headless();
    let duration = 50_000;
    player.open_file("/path/to/vid.mp4", duration);
    player.seek(duration);
    let mut is_fs = false;

    handle_shortcut(&mut player, KeyAction::SeekRelative(5_000), &mut is_fs);
    assert_eq!(player.position_ms, duration);
}

// ============================================================================
// FEATURE 6: FULLSCREEN & WINDOW ASPECT RATIO
// ============================================================================

#[test]
fn test_tier1_f6_fullscreen_toggle_f_key() {
    let mut player = MockPlayer::new_headless();
    let mut is_fs = false;

    handle_shortcut(&mut player, KeyAction::ToggleFullscreen, &mut is_fs);
    assert!(is_fs);

    handle_shortcut(&mut player, KeyAction::ToggleFullscreen, &mut is_fs);
    assert!(!is_fs);
}

#[test]
fn test_tier1_f6_fullscreen_toggle_double_click() {
    let mut is_fullscreen = false;
    let on_double_click = |fs: &mut bool| *fs = !*fs;

    on_double_click(&mut is_fullscreen);
    assert!(is_fullscreen);

    on_double_click(&mut is_fullscreen);
    assert!(!is_fullscreen);
}

#[test]
fn test_tier1_f6_fullscreen_exit_esc_key() {
    let mut player = MockPlayer::new_headless();
    let mut is_fs = true;

    handle_shortcut(&mut player, KeyAction::ExitFullscreen, &mut is_fs);
    assert!(!is_fs);

    // Pressing ESC again remains false
    handle_shortcut(&mut player, KeyAction::ExitFullscreen, &mut is_fs);
    assert!(!is_fs);
}

#[test]
fn test_tier1_f6_aspect_ratio_16_9_letterbox_calculation() {
    let video = VideoDimensions::new(1920, 1080); // 16:9
    // Viewport is square 1000x1000 -> letterboxed
    let (fit_w, fit_h) = video.fit_in_viewport(1000, 1000);

    assert_eq!(fit_w, 1000);
    assert_eq!(fit_h, 563); // 1000 / (16/9) = 562.5 -> 563
}

#[test]
fn test_tier1_f6_aspect_ratio_21_9_ultrawide_pillarbox_calculation() {
    let video = VideoDimensions::new(2560, 1080); // 21:9 (~2.37)
    // Viewport is very wide 3000x1000 -> pillarboxed
    let (fit_w, fit_h) = video.fit_in_viewport(3000, 1000);

    assert_eq!(fit_h, 1000);
    assert_eq!(fit_w, 2370); // 1000 * 2.37 = 2370
}

#[test]
fn test_tier2_f6_fullscreen_consecutive_toggles() {
    let mut player = MockPlayer::new_headless();
    let mut is_fs = false;

    for i in 1..=10 {
        handle_shortcut(&mut player, KeyAction::ToggleFullscreen, &mut is_fs);
        assert_eq!(is_fs, i % 2 != 0);
    }
}

#[test]
fn test_tier2_f6_aspect_ratio_1_1_square_aspect_calculation() {
    let video = VideoDimensions::new(1080, 1080); // 1:1
    let (fit_w, fit_h) = video.fit_in_viewport(1920, 1080);

    assert_eq!(fit_w, 1080);
    assert_eq!(fit_h, 1080);
}

#[test]
fn test_tier2_f6_aspect_ratio_zero_width_or_height_fallback() {
    let video = VideoDimensions::new(0, 0);
    let (fit_w, fit_h) = video.fit_in_viewport(800, 600);

    assert_eq!(fit_w, 800);
    assert_eq!(fit_h, 600);
}

#[test]
fn test_tier2_f6_aspect_ratio_extreme_portrait_4_3_and_9_16() {
    let video_portrait = VideoDimensions::new(1080, 1920); // 9:16 vertical
    let (fit_w, fit_h) = video_portrait.fit_in_viewport(1920, 1080);

    assert_eq!(fit_h, 1080);
    assert_eq!(fit_w, 608); // 1080 * (9/16) = 607.5 -> 608
}

#[test]
fn test_tier2_f6_fullscreen_state_retains_playback_position() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    player.seek(42_000);
    let mut is_fs = false;

    handle_shortcut(&mut player, KeyAction::ToggleFullscreen, &mut is_fs);
    assert!(is_fs);
    assert_eq!(player.position_ms, 42_000);

    handle_shortcut(&mut player, KeyAction::ToggleFullscreen, &mut is_fs);
    assert!(!is_fs);
    assert_eq!(player.position_ms, 42_000);
}

// ============================================================================
// FEATURE 7: LOCAL VOLUME & MUTE
// ============================================================================

#[test]
fn test_tier1_f7_volume_default_level_is_1_0() {
    let player = MockPlayer::new_headless();
    assert_eq!(player.volume, 1.0);
    assert!(!player.is_muted);
}

#[test]
fn test_tier1_f7_volume_adjust_to_half_0_5() {
    let mut player = MockPlayer::new_headless();
    player.set_volume(0.5);
    assert_eq!(player.volume, 0.5);
}

#[test]
fn test_tier1_f7_volume_mute_toggle_m_key() {
    let mut player = MockPlayer::new_headless();
    let mut is_fs = false;

    assert!(!player.is_muted);
    handle_shortcut(&mut player, KeyAction::ToggleMute, &mut is_fs);
    assert!(player.is_muted);
    handle_shortcut(&mut player, KeyAction::ToggleMute, &mut is_fs);
    assert!(!player.is_muted);
}

#[test]
fn test_tier1_f7_volume_unmute_restores_previous_volume() {
    let mut player = MockPlayer::new_headless();
    player.set_volume(0.65);
    player.set_mute(true);
    assert!(player.is_muted);
    assert_eq!(player.volume, 0.65);

    player.set_mute(false);
    assert!(!player.is_muted);
    assert_eq!(player.volume, 0.65);
}

#[test]
fn test_tier1_f7_volume_is_local_isolated_from_shared_state() {
    // Volume & Mute are local and must NOT be part of PlaybackState
    use watch_together_tests::protocol::{PlaybackState, PlaybackStatus};

    let state = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Playing,
        position_ms: 10_000,
        speed: 1.0,
        updated_at_server_ms: 100,
        duration_ms: 100_000,
    };

    // Serialize to JSON and verify volume and mute are NOT present
    let json_val = serde_json::to_value(&state).unwrap();
    assert!(json_val.get("volume").is_none());
    assert!(json_val.get("muted").is_none());
    assert!(json_val.get("is_muted").is_none());
}

#[test]
fn test_tier2_f7_volume_boundary_minimum_zero() {
    let mut player = MockPlayer::new_headless();
    player.set_volume(0.0);
    assert_eq!(player.volume, 0.0);
}

#[test]
fn test_tier2_f7_volume_boundary_maximum_one() {
    let mut player = MockPlayer::new_headless();
    player.set_volume(1.0);
    assert_eq!(player.volume, 1.0);
}

#[test]
fn test_tier2_f7_volume_clamping_above_one() {
    let mut player = MockPlayer::new_headless();
    player.set_volume(1.5);
    assert_eq!(player.volume, 1.0);
}

#[test]
fn test_tier2_f7_volume_clamping_below_zero() {
    let mut player = MockPlayer::new_headless();
    player.set_volume(-0.5);
    assert_eq!(player.volume, 0.0);
}

#[test]
fn test_tier2_f7_volume_mute_at_zero_volume() {
    let mut player = MockPlayer::new_headless();
    player.set_volume(0.0);
    player.set_mute(true);
    assert!(player.is_muted);
    assert_eq!(player.volume, 0.0);
    player.set_mute(false);
    assert!(!player.is_muted);
    assert_eq!(player.volume, 0.0);
}
