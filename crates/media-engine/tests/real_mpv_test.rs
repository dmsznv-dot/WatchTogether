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

/// Canvas path budget: a 1080p60 clip must be delivered at (nearly) full rate by the
/// software renderer, which the Windows build uses. Set WT_TEST_HD60 to `hd60.mp4`
/// from scripts/create_player_fixture.py.
#[test]
#[ignore = "requires libmpv and WT_TEST_HD60 fixture"]
fn software_renderer_keeps_up_with_1080p60() {
    use media_engine::mpv::{MAX_HEIGHT, MAX_WIDTH};
    let path = std::env::var("WT_TEST_HD60").expect("set WT_TEST_HD60");
    let hwdec = std::env::var("WT_TEST_HWDEC").unwrap_or_else(|_| "auto-safe".into());
    let player = std::sync::Arc::new(
        MpvPlayer::new_with_options(&[("vo", "libmpv"), ("ao", "null"), ("hwdec", &hwdec)])
            .unwrap(),
    );
    let mut renderer = player.software_renderer().unwrap();
    player.open_file(&path).unwrap();
    player.play().unwrap();
    let (w, h) = (MAX_WIDTH, MAX_HEIGHT);
    let stride = w as usize * 4;
    let mut storage = vec![0u32; w as usize * h as usize];
    let frame = unsafe {
        std::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<u8>(), storage.len() * 4)
    };
    // Skip startup (file open, decoder init) before measuring.
    let warmup = Instant::now() + Duration::from_secs(1);
    while Instant::now() < warmup {
        if renderer.has_new_frame() {
            renderer.render_into(w, h, stride, frame).unwrap();
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let drops_before: i64 = player
        .property_string("frame-drop-count")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let started = Instant::now();
    let (mut frames, mut render_total, mut render_max) = (0u32, Duration::ZERO, Duration::ZERO);
    while started.elapsed() < Duration::from_secs(3) {
        if renderer.has_new_frame() {
            let t = Instant::now();
            renderer.render_into(w, h, stride, frame).unwrap();
            let spent = t.elapsed();
            render_total += spent;
            render_max = render_max.max(spent);
            frames += 1;
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let fps = frames as f64 / started.elapsed().as_secs_f64();
    let avg_ms = render_total.as_secs_f64() * 1000.0 / frames.max(1) as f64;
    let drops: i64 = player
        .property_string("frame-drop-count")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
        - drops_before;
    println!(
        "1080p60 software render: {fps:.1} fps, render avg {avg_ms:.1} ms, max {:.1} ms, \
         decoder drops {drops}, hwdec {:?}",
        render_max.as_secs_f64() * 1000.0,
        player.property_string("hwdec-current")
    );
    assert!(avg_ms < 12.0, "rendering 1080p takes {avg_ms:.1} ms per frame");
    // Only Windows shows video through this software path; elsewhere the delivered
    // rate mostly reflects the (often weak, GPU-less) CI machine's decoder.
    if cfg!(windows) {
        assert!(fps >= 50.0, "only {fps:.1} fps delivered for 60 fps video");
    } else if fps < 50.0 {
        println!("note: {fps:.1} fps on this machine; the fps gate applies to Windows only");
    }
}

/// The `I` key toggles mpv's stats overlay; it must actually be drawn into the frames.
#[test]
#[ignore = "requires libmpv and WT_TEST_MEDIA fixture"]
fn stats_overlay_is_drawn_into_rendered_frames() {
    let path = std::env::var("WT_TEST_MEDIA").expect("set WT_TEST_MEDIA");
    let player = std::sync::Arc::new(
        MpvPlayer::new_with_options(&[("vo", "libmpv"), ("ao", "null")]).unwrap(),
    );
    let mut renderer = player.software_renderer().unwrap();
    player.open_file(&path).unwrap();
    std::thread::sleep(Duration::from_millis(800));
    player.pause().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let (w, h) = (640, 360);
    let corner = |frame: &[u8]| -> Vec<u8> {
        (0..h / 3).flat_map(|y| frame[y * w * 4..(y * w + w / 2) * 4].to_vec()).collect()
    };
    let before = corner(&renderer.render(w as u32, h as u32).unwrap());
    player.toggle_stats().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        let after = corner(&renderer.render(w as u32, h as u32).unwrap());
        let changed = before.iter().zip(&after).filter(|(a, b)| a.abs_diff(**b) > 40).count();
        if changed > 200 {
            return;
        }
    }
    panic!("stats overlay never appears in the rendered frame");
}

/// An audio file next to the movie (e.g. a dub) becomes an extra, selected audio track.
/// Uses dub.m4a, generated next to WT_TEST_MEDIA by scripts/create_player_fixture.py.
#[test]
#[ignore = "requires libmpv and WT_TEST_MEDIA fixture"]
fn real_decoder_adds_and_plays_an_external_audio_track() {
    let path = std::env::var("WT_TEST_MEDIA").expect("set WT_TEST_MEDIA");
    let dub = std::path::Path::new(&path).with_file_name("dub.m4a");
    let player = MpvPlayer::new_with_options(&[("vo", "null"), ("ao", "null")]).unwrap();
    player.open_file(&path).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while player.audio_tracks().unwrap().len() < 2 {
        assert!(Instant::now() < deadline, "embedded audio tracks never appear");
        std::thread::sleep(Duration::from_millis(50));
    }
    let embedded = player.audio_tracks().unwrap().len();
    let id = player.add_external_audio(dub.to_str().unwrap(), None, None).unwrap();
    let tracks = player.audio_tracks().unwrap();
    assert_eq!(tracks.len(), embedded + 1);
    let added = tracks.iter().find(|t| t.id == id).expect("added track listed");
    assert!(added.is_selected && added.is_external);
    assert_eq!(player.property_string("aid").as_deref(), Some(id.to_string().as_str()));
    // Switching back to an embedded track and to the dub again both work.
    player.select_audio(1).unwrap();
    player.select_audio(id).unwrap();
    assert!(player.audio_tracks().unwrap().iter().any(|t| t.id == id && t.is_selected));
    assert!(player.add_external_audio("/tmp/notes.txt", None, None).is_err());
}
