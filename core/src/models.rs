use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Folder {
    pub id: i64,
    pub path: String,
    pub video_count: i64,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub video_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoTag {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub video_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Video {
    pub id: i64,
    pub path: String,
    pub folder_id: i64,
    pub title: String,
    pub size: i64,
    pub modified: i64,
    pub watched: bool,
    pub tags: Vec<VideoTag>,
    pub playlist_ids: Vec<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct VideoFilter {
    /// Case-insensitive substring match on title or path.
    pub search: Option<String>,
    /// Video must have ALL of these tags.
    pub tag_ids: Vec<i64>,
    /// Restrict to a playlist (result is then ordered by playlist position).
    pub playlist_id: Option<i64>,
    pub watched: Option<bool>,
    pub folder_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ScanReport {
    pub added: usize,
    pub removed: usize,
    pub total: usize,
    /// Newly added videos that matched at least one automatic rule.
    pub rule_matches: usize,
}

/// Automatic rule: when a *new* video's full path matches `pattern`, all linked
/// tags and playlists are applied to it.
#[derive(Debug, Clone, Serialize)]
pub struct Rule {
    pub id: i64,
    pub name: String,
    pub pattern: String,
    pub case_insensitive: bool,
    pub enabled: bool,
    pub tag_ids: Vec<i64>,
    pub playlist_ids: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleInput {
    pub name: String,
    pub pattern: String,
    #[serde(default = "yes")]
    pub case_insensitive: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub tag_ids: Vec<i64>,
    #[serde(default)]
    pub playlist_ids: Vec<i64>,
}

fn yes() -> bool {
    true
}
