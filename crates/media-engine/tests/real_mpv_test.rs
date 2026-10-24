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

/// Offscreen OpenGL context for driving the GPU frame renderer in tests (macOS: CGL).
#[cfg(target_os = "macos")]
mod cgl {
    use std::ffi::{c_char, c_void};
    #[link(name = "OpenGL", kind = "framework")]
    extern "C" {
        fn CGLChoosePixelFormat(attrs: *const i32, pix: *mut *mut c_void, count: *mut i32) -> i32;
        fn CGLCreateContext(pix: *mut c_void, share: *mut c_void, ctx: *mut *mut c_void) -> i32;
        fn CGLSetCurrentContext(ctx: *mut c_void) -> i32;
        fn CGLDestroyPixelFormat(pix: *mut c_void) -> i32;
    }
    extern "C" {
        fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    }
    pub unsafe extern "C" fn get_proc(_: *mut c_void, name: *const c_char) -> *mut c_void {
        dlsym(-2isize as *mut c_void, name) // RTLD_DEFAULT
    }
    /// Makes a 3.2 core context current on this thread (any renderer, software included).
    pub fn make_current() {
        const PROFILE: i32 = 99;
        const CORE_3_2: i32 = 0x3200;
        let attrs = [PROFILE, CORE_3_2, 0];
        unsafe {
            let (mut pix, mut n, mut ctx) = (std::ptr::null_mut(), 0, std::ptr::null_mut());
            assert_eq!(CGLChoosePixelFormat(attrs.as_ptr(), &mut pix, &mut n), 0);
            assert_eq!(CGLCreateContext(pix, std::ptr::null_mut(), &mut ctx), 0);
            CGLDestroyPixelFormat(pix);
            assert_eq!(CGLSetCurrentContext(ctx), 0);
        }
    }
}

fn half_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((bits >> 10) & 0x1f) as i32;
    let frac = (bits & 0x3ff) as f32;
    sign * match exp {
        0 => frac / 1024.0 * 2f32.powi(-14),
        31 => f32::INFINITY,
        _ => (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}

/// The GPU renderer keeps HDR video's real luminance for HDR displays (values above SDR
/// white) and tone-maps it into the normal range for SDR displays. Uses hdr10.mp4.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires libmpv and WT_TEST_MEDIA fixture"]
fn gpu_renderer_outputs_hdr_luminance_or_tone_maps_to_sdr() {
    use media_engine::mpv::DynamicRange;
    let path = std::env::var("WT_TEST_MEDIA").expect("set WT_TEST_MEDIA");
    let hdr10 = std::path::Path::new(&path).with_file_name("hdr10.mp4");
    let player = std::sync::Arc::new(
        MpvPlayer::new_with_options(&[("vo", "libmpv"), ("ao", "null")]).unwrap(),
    );
    cgl::make_current();
    let mut renderer = unsafe { player.gl_frame_renderer(cgl::get_proc, std::ptr::null_mut()) }.unwrap();
    println!("GPU renderer: {}", renderer.describe());
    player.open_file(hdr10.to_str().unwrap()).unwrap();
    player.pause().unwrap();
    let (w, h) = (640u32, 360u32);
    let mut frame = vec![0u8; (w * h * 8) as usize];
    let render = |renderer: &mut media_engine::mpv::GlFrameRenderer, frame: &mut Vec<u8>| {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !renderer.has_new_frame() {
            assert!(Instant::now() < deadline, "no frame from the GPU renderer");
            std::thread::sleep(Duration::from_millis(20));
        }
        // A couple of renders so the paused frame is the decoded picture, not a blank first pass.
        for _ in 0..3 {
            renderer.render_into(w, h, frame).unwrap();
            std::thread::sleep(Duration::from_millis(50));
            let _ = renderer.has_new_frame();
        }
    };
    let probe = |x: u32, y: u32| (y * w + x) as usize;
    let (left, mid, right, bottom) = (probe(w / 6, h / 3), probe(w / 2, h / 3), probe(w * 5 / 6, h / 3), probe(w / 2, h - 5));

    render(&mut renderer, &mut frame);
    let sdr = |i: usize| frame[i * 4];
    let (sl, sm, sr, sb) = (sdr(left), sdr(mid), sdr(right), sdr(bottom));
    println!("SDR (8-bit red): 1000 nits {sl}, 203 nits {sm}, 100 nits {sr}, black {sb}");
    assert!(sl >= sm && sm > sr && sr > sb, "SDR tone mapping keeps the bars in order");
    assert!(sb < 20, "bottom of the picture comes out at the bottom (row order)");

    renderer.set_dynamic_range(DynamicRange::Hdr { peak_nits: 1000.0 }).unwrap();
    render(&mut renderer, &mut frame);
    let hdr = |i: usize| half_to_f32(u16::from_le_bytes([frame[i * 8], frame[i * 8 + 1]]));
    let (hl, hm, hr, hb) = (hdr(left), hdr(mid), hdr(right), hdr(bottom));
    println!("HDR (linear, 1.0 = 203 nits): 1000 nits {hl:.2}, 203 nits {hm:.2}, 100 nits {hr:.2}, black {hb:.3}");
    assert!((hl - 1000.0 / 203.0).abs() < 0.8, "1000-nit bar should be ~4.9x SDR white, got {hl}");
    assert!((hm - 1.0).abs() < 0.15, "203-nit bar should be ~SDR white, got {hm}");
    assert!((hr - 100.0 / 203.0).abs() < 0.1, "100-nit bar should be ~0.49, got {hr}");
    assert!(hb < 0.02, "black stays black, got {hb}");
}
