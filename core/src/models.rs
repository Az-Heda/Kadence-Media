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
    /// File creation time (unix seconds; falls back to modified time where unsupported).
    pub created: i64,
    pub watched: bool,
    /// Hidden from the library unless "show hidden" is on.
    pub hidden: bool,
    /// 0 = unrated, otherwise 0.5..=5.0 in steps of 0.5.
    pub rating: f64,
    /// Seconds. None = not indexed yet; Some(0.0) = could not be determined.
    pub duration: Option<f64>,
    /// True once a thumbnail image exists for this video.
    pub thumb: bool,
    pub tags: Vec<VideoTag>,
    pub playlist_ids: Vec<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    /// Playlist order when a playlist is selected, otherwise by title.
    #[default]
    Default,
    Title,
    Path,
    Created,
    Size,
    /// Videos without a known duration always sort last.
    Duration,
    /// Unrated videos always sort last.
    Rating,
}

/// How hidden videos take part in a listing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HiddenMode {
    /// Normal library: hidden videos are left out.
    #[default]
    Exclude,
    /// Only the hidden videos.
    Only,
    /// Everything, hidden or not (internal operations such as playlist reordering).
    Any,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct VideoFilter {
    pub hidden: HiddenMode,
    pub sort_by: SortBy,
    pub sort_desc: bool,
    /// Case-insensitive substring match on title or path.
    pub search: Option<String>,
    /// Video must have ALL of these tags.
    pub tag_ids: Vec<i64>,
    /// Restrict to a playlist (result is then ordered by playlist position).
    pub playlist_id: Option<i64>,
    pub watched: Option<bool>,
    pub folder_id: Option<i64>,
}

/// One page of videos, already filtered and sorted.
#[derive(Debug, Clone, Serialize)]
pub struct VideoPage {
    pub items: Vec<Video>,
    /// Number of videos matching the filter across all pages.
    pub total: usize,
    /// The page actually returned (0-based; clamped to the last page).
    pub page: usize,
    pub page_size: usize,
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
    /// Matching videos are hidden from the library.
    pub hide: bool,
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
    pub hide: bool,
    #[serde(default)]
    pub tag_ids: Vec<i64>,
    #[serde(default)]
    pub playlist_ids: Vec<i64>,
}

fn yes() -> bool {
    true
}

// ------------------------------------------------------------- statistics

#[derive(Debug, Clone, Serialize)]
pub struct Bucket {
    pub key: String,
    pub count: i64,
}

/// Totals for one folder, tag or playlist (hidden videos excluded).
#[derive(Debug, Clone, Serialize)]
pub struct GroupStat {
    pub id: i64,
    pub name: String,
    /// Tags only.
    pub color: Option<String>,
    pub videos: i64,
    pub watched: i64,
    pub size: i64,
    pub duration: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoBrief {
    pub id: i64,
    pub title: String,
    pub size: i64,
    pub duration: Option<f64>,
    pub rating: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonthCount {
    /// "YYYY-MM"
    pub month: String,
    pub count: i64,
}

/// Library statistics. Everything except `hidden` ignores hidden videos.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Stats {
    pub videos: i64,
    pub hidden: i64,
    pub watched: i64,
    pub total_size: i64,
    /// Seconds, over videos whose duration is known.
    pub total_duration: f64,
    pub watched_duration: f64,
    /// Videos whose duration has not been read yet.
    pub unindexed: i64,
    pub rated: i64,
    /// Average over rated videos (0 when none).
    pub avg_rating: f64,
    /// Videos per rating, 0.5 to 5 stars in half steps (10 entries).
    pub rating_distribution: Vec<i64>,
    pub duration_buckets: Vec<Bucket>,
    pub folders: Vec<GroupStat>,
    pub tags: Vec<GroupStat>,
    pub playlists: Vec<GroupStat>,
    pub largest: Vec<VideoBrief>,
    pub longest: Vec<VideoBrief>,
    pub top_rated: Vec<VideoBrief>,
    /// Videos first seen per month, last 12 months, oldest first.
    pub added_by_month: Vec<MonthCount>,
}
