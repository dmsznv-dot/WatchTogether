use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackType {
    Audio,
    Subtitle,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaTrack {
    pub id: i64,
    pub track_type: TrackType,
    pub title: Option<String>,
    pub lang: Option<String>,
    pub is_selected: bool,
    pub is_external: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
}

impl MediaTrack {
    pub fn new_audio(id: i64, title: Option<String>, lang: Option<String>, is_selected: bool) -> Self {
        Self {
            id,
            track_type: TrackType::Audio,
            title,
            lang,
            is_selected,
            is_external: false,
            codec: None,
        }
    }

    pub fn new_subtitle(
        id: i64,
        title: Option<String>,
        lang: Option<String>,
        is_selected: bool,
        is_external: bool,
    ) -> Self {
        Self {
            id,
            track_type: TrackType::Subtitle,
            title,
            lang,
            is_selected,
            is_external,
            codec: None,
        }
    }
}

/// Audio files that can be added next to a video as an extra (external) track.
pub const EXTERNAL_AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "aac", "ac3", "eac3", "dts", "thd", "flac", "wav", "ogg", "oga", "opus", "mka",
    "wma", "mp2",
];

/// Lower-case extension of `path` if it is a supported external audio file.
pub fn external_audio_extension(path: &str) -> Result<String, crate::MediaEngineError> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if EXTERNAL_AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Ok(ext)
    } else {
        Err(crate::MediaEngineError::UnsupportedAudioExtension(ext))
    }
}
