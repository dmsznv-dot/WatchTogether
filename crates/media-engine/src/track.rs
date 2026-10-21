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
