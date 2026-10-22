//! Opt-in decoder integration test. Set WT_TEST_MEDIA to a two-audio-track file.
use media_engine::{MediaPlayer, MpvPlayer, PlayerEvent};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires libmpv and WT_TEST_MEDIA fixture"]
fn embedded_renderer_produces_visible_pixels_without_a_native_window() {
    let path = std::env::var("WT_TEST_MEDIA").expect("set WT_TEST_MEDIA");
    let player = std::sync::Arc::new(
        MpvPlayer::new_with_options(&[("vo", "libmpv"), ("ao", "null")]).unwrap(),
    );
    let mut renderer = player.software_renderer().unwrap();
    player.open_file(&path).unwrap();
    player.play().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let pixels = renderer.render(320, 180).unwrap();
        assert_eq!(pixels.len(), 320 * 180 * 4);
        assert!(pixels.chunks_exact(4).all(|p| p[3] == 255));
        if pixels
            .chunks_exact(4)
            .any(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    panic!("embedded renderer never produces visible video pixels");
}

#[test]
#[ignore = "requires libmpv and WT_TEST_MEDIA fixture"]
fn real_decoder_announces_tracks_and_switches_audio_and_subtitles() {
    let path = std::env::var("WT_TEST_MEDIA").expect("set WT_TEST_MEDIA");
    let player = MpvPlayer::new_with_options(&[("vo", "null"), ("ao", "null")]).unwrap();
    let mut events = player.subscribe_events();
    player.open_file(&path).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut announced = false;
    while Instant::now() < deadline {
        if let Ok(PlayerEvent::TracksChange { tracks }) = events.try_recv() {
            if tracks
                .iter()
                .filter(|t| t.track_type == media_engine::TrackType::Audio)
                .count()
                == 2
            {
                announced = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        announced,
        "decoder must announce tracks after asynchronous loading"
    );
    let tracks = player.audio_tracks().unwrap();
    assert_eq!(tracks.len(), 2);
    assert!(tracks.iter().all(|t| t.codec.is_some()));
    for track in tracks {
        player.select_audio(track.id).unwrap();
        assert!(player
            .audio_tracks()
            .unwrap()
            .iter()
            .any(|t| t.id == track.id && t.is_selected));
    }
    let embedded = player
        .subtitle_tracks()
        .unwrap()
        .into_iter()
        .find(|t| !t.is_external)
        .expect("fixture must contain embedded ASS subtitles");
    player.select_subtitle(embedded.id).unwrap();
    assert!(player
        .subtitle_tracks()
        .unwrap()
        .iter()
        .any(|t| t.id == embedded.id && t.is_selected));
    let sub_path = std::path::Path::new(&path).with_extension("srt");
    let id = player
        .add_external_subtitle(sub_path.to_str().unwrap(), None, Some("rus"))
        .unwrap();
    assert!(player
        .subtitle_tracks()
        .unwrap()
        .iter()
        .any(|t| t.id == id && t.is_selected && t.is_external));
    assert!(player
        .subtitle_tracks()
        .unwrap()
        .iter()
        .any(|t| t.id == id && t.lang.as_deref() == Some("rus")));
    player.select_subtitle(-1).unwrap();
    assert!(player
        .subtitle_tracks()
        .unwrap()
        .iter()
        .all(|t| !t.is_selected));
    player.select_subtitle(id).unwrap();
    assert!(player
        .subtitle_tracks()
        .unwrap()
        .iter()
        .any(|t| t.id == id && t.is_selected));
}
