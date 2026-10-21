use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Media track representation (audio or subtitle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockMediaTrack {
    pub id: i64,
    pub track_type: String, // "audio" or "sub"
    pub title: Option<String>,
    pub lang: Option<String>,
    pub is_selected: bool,
    pub is_external: bool,
}

/// Aspect ratio helper according to ORIGINAL_REQUEST.md § R1
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoDimensions {
    pub width: u32,
    pub height: u32,
}

impl VideoDimensions {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0 {
            16.0 / 9.0 // default fallback
        } else {
            self.width as f64 / self.height as f64
        }
    }

    pub fn fit_in_viewport(&self, viewport_w: u32, viewport_h: u32) -> (u32, u32) {
        if self.width == 0 || self.height == 0 || viewport_w == 0 || viewport_h == 0 {
            return (viewport_w, viewport_h);
        }
        let video_ar = self.aspect_ratio();
        let viewport_ar = viewport_w as f64 / viewport_h as f64;

        if viewport_ar > video_ar {
            // Viewport is wider than video -> pillarboxed (black bars on left/right)
            let fit_h = viewport_h;
            let fit_w = (viewport_h as f64 * video_ar).round() as u32;
            (fit_w, fit_h)
        } else {
            // Viewport is taller than video -> letterboxed (black bars on top/bottom)
            let fit_w = viewport_w;
            let fit_h = (viewport_w as f64 / video_ar).round() as u32;
            (fit_w, fit_h)
        }
    }
}

/// Keyboard shortcuts dispatcher according to ORIGINAL_REQUEST.md § R1
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    SpaceTogglePlayPause,
    SeekRelative(i64), // milliseconds
    ToggleFullscreen,
    ToggleMute,
    ExitFullscreen,
}

pub fn handle_shortcut(player: &mut MockPlayer, action: KeyAction, is_fullscreen: &mut bool) {
    match action {
        KeyAction::SpaceTogglePlayPause => {
            if player.is_playing {
                player.pause();
            } else {
                player.play();
            }
        }
        KeyAction::SeekRelative(delta_ms) => {
            let cur = player.position_ms as i64;
            let target = (cur + delta_ms).max(0) as u64;
            player.seek(target);
        }
        KeyAction::ToggleFullscreen => {
            *is_fullscreen = !*is_fullscreen;
        }
        KeyAction::ExitFullscreen => {
            *is_fullscreen = false;
        }
        KeyAction::ToggleMute => {
            player.set_mute(!player.is_muted);
        }
    }
}

/// Headless/in-memory mock media player implementing decoupled playback control.
#[derive(Debug, Clone)]
pub struct MockPlayer {
    pub is_headless: bool,
    pub current_file: Option<String>,
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub speed: f32,
    pub volume: f64,
    pub is_muted: bool,
    pub audio_tracks: Vec<MockMediaTrack>,
    pub subtitle_tracks: Vec<MockMediaTrack>,
}

impl MockPlayer {
    pub fn new_headless() -> Self {
        Self {
            is_headless: true,
            current_file: None,
            is_playing: false,
            position_ms: 0,
            duration_ms: 0,
            speed: 1.0,
            volume: 1.0,
            is_muted: false,
            audio_tracks: vec![
                MockMediaTrack {
                    id: 1,
                    track_type: "audio".into(),
                    title: Some("English Stereo".into()),
                    lang: Some("eng".into()),
                    is_selected: true,
                    is_external: false,
                },
                MockMediaTrack {
                    id: 2,
                    track_type: "audio".into(),
                    title: Some("Director Commentary".into()),
                    lang: Some("eng".into()),
                    is_selected: false,
                    is_external: false,
                },
            ],
            subtitle_tracks: vec![MockMediaTrack {
                id: 1,
                track_type: "sub".into(),
                title: Some("English Full".into()),
                lang: Some("eng".into()),
                is_selected: false,
                is_external: false,
            }],
        }
    }

    pub fn open_file(&mut self, path_or_url: &str, duration_ms: u64) {
        self.current_file = Some(path_or_url.to_string());
        self.duration_ms = duration_ms;
        self.position_ms = 0;
        self.is_playing = false;
    }

    pub fn play(&mut self) {
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn seek(&mut self, target_ms: u64) {
        if self.duration_ms > 0 {
            self.position_ms = target_ms.min(self.duration_ms);
        } else {
            self.position_ms = target_ms;
        }
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn set_volume(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_mute(&mut self, muted: bool) {
        self.is_muted = muted;
    }

    pub fn select_audio(&mut self, track_id: i64) -> bool {
        if !self.audio_tracks.iter().any(|t| t.id == track_id) {
            return false;
        }
        for track in &mut self.audio_tracks {
            track.is_selected = track.id == track_id;
        }
        true
    }

    pub fn select_subtitle(&mut self, track_id: i64) -> bool {
        if track_id == -1 {
            for track in &mut self.subtitle_tracks {
                track.is_selected = false;
            }
            return true;
        }
        if !self.subtitle_tracks.iter().any(|t| t.id == track_id) {
            return false;
        }
        for track in &mut self.subtitle_tracks {
            track.is_selected = track.id == track_id;
        }
        true
    }

    pub fn add_external_subtitle(&mut self, _path: &str, title: &str, lang: &str) -> i64 {
        let new_id = (self.subtitle_tracks.len() as i64) + 1;
        self.subtitle_tracks.push(MockMediaTrack {
            id: new_id,
            track_type: "sub".into(),
            title: Some(title.to_string()),
            lang: Some(lang.to_string()),
            is_selected: true,
            is_external: true,
        });
        new_id
    }
}

/// Generates a deterministic synthetic video media file with a repeating pattern.
pub fn create_synthetic_media_file(path: &Path, size_bytes: usize) -> std::io::Result<Vec<u8>> {
    let mut data = Vec::with_capacity(size_bytes);
    for i in 0..size_bytes {
        // Deterministic pseudo-random byte generation: (i * 37) % 256
        data.push(((i * 37) % 256) as u8);
    }

    let mut file = File::create(path)?;
    file.write_all(&data)?;
    file.sync_all()?;
    Ok(data)
}

/// Computes the hex SHA-256 digest of a byte slice.
pub fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    hex_encode(&digest)
}

/// Computes the hex SHA-256 digest of a range [start..=end] within a byte slice.
pub fn compute_range_sha256(bytes: &[u8], start: usize, end: usize) -> String {
    let slice = &bytes[start..=end];
    compute_sha256(slice)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
