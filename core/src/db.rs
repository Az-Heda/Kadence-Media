use crate::error::{Error, Result};
use crate::models::*;
use crate::scan::{scan_dir, ScannedFile};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS folders (
    id   INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS videos (
    id        INTEGER PRIMARY KEY,
    path      TEXT NOT NULL UNIQUE,
    folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    title     TEXT NOT NULL,
    size      INTEGER NOT NULL DEFAULT 0,
    modified  INTEGER NOT NULL DEFAULT 0,
    created   INTEGER NOT NULL DEFAULT 0,
    rating    INTEGER NOT NULL DEFAULT 0,
    duration  REAL,
    thumb     INTEGER,
    watched   INTEGER NOT NULL DEFAULT 0,
    added_at  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);
CREATE INDEX IF NOT EXISTS idx_videos_folder ON videos(folder_id);
-- Thumbnails live in their own table so listing videos never loads image data,
-- and are removed together with their video.
CREATE TABLE IF NOT EXISTS thumbnails (
    video_id INTEGER PRIMARY KEY REFERENCES videos(id) ON DELETE CASCADE,
    data     BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS tags (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color TEXT NOT NULL DEFAULT '#d4a017'
);
CREATE TABLE IF NOT EXISTS video_tags (
    video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES tags(id)   ON DELETE CASCADE,
    PRIMARY KEY (video_id, tag_id)
);
CREATE TABLE IF NOT EXISTS playlists (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE IF NOT EXISTS playlist_items (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    video_id    INTEGER NOT NULL REFERENCES videos(id)    ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, video_id)
);
CREATE TABLE IF NOT EXISTS rules (
    id               INTEGER PRIMARY KEY,
    name             TEXT NOT NULL,
    pattern          TEXT NOT NULL,
    case_insensitive INTEGER NOT NULL DEFAULT 1,
    enabled          INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS rule_tags (
    rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
    PRIMARY KEY (rule_id, tag_id)
);
CREATE TABLE IF NOT EXISTS rule_playlists (
    rule_id     INTEGER NOT NULL REFERENCES rules(id)     ON DELETE CASCADE,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    PRIMARY KEY (rule_id, playlist_id)
);
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Default values for settings that have not been changed by the user.
pub const DEFAULT_SETTINGS: &[(&str, &str)] = &[
    ("volume_max", "300"),
    ("volume", "100"),
    ("primary_color", "#d4a017"),
    ("embed_video", "true"),
    ("mpv_path", ""),
    ("library_view", "grid"),
    ("default_folder_id", ""),
    ("escape_action", "stop"),
    ("grid_page_size", "60"),
    ("table_page_size", "20"),
    ("col_widths", ""),
    ("saved_themes", ""),
    ("table_columns", ""),
    ("bg_color", ""),
    ("surface_color", ""),
    ("surface2_color", ""),
    ("border_color", ""),
    ("text_color", ""),
    ("muted_color", ""),
];

pub struct Library {
    pub(crate) conn: Connection,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(SCHEMA)?;
        // Upgrade databases created before these columns existed.
        for (col, ddl) in [
            ("created", "ALTER TABLE videos ADD COLUMN created INTEGER NOT NULL DEFAULT 0"),
            ("rating", "ALTER TABLE videos ADD COLUMN rating INTEGER NOT NULL DEFAULT 0"),
            ("duration", "ALTER TABLE videos ADD COLUMN duration REAL"),
            ("thumb", "ALTER TABLE videos ADD COLUMN thumb INTEGER"),
        ] {
            let exists: bool = conn
                .prepare("SELECT 1 FROM pragma_table_info('videos') WHERE name = ?1")?
                .exists(params![col])?;
            if !exists {
                conn.execute_batch(ddl)?;
            }
        }
        // Thumbnails used to be files on disk: anything flagged done without an image is redone.
        conn.execute(
            "UPDATE videos SET thumb = NULL WHERE thumb = 1 AND id NOT IN (SELECT video_id FROM thumbnails)",
            [],
        )?;
        Ok(Library { conn })
    }

    // ---------------------------------------------------------------- folders

    pub fn add_folder(&self, path: &str) -> Result<Folder> {
        let p = Path::new(path);
        if !p.is_dir() {
            return Err(Error::Msg(format!("'{path}' is not a directory")));
        }
        let canonical = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let canonical = strip_verbatim(&canonical.to_string_lossy());
        self.conn.execute(
            "INSERT OR IGNORE INTO folders(path) VALUES (?1)",
            params![canonical],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM folders WHERE path = ?1",
            params![canonical],
            |r| r.get(0),
        )?;
        Ok(Folder { id, path: canonical, video_count: 0, is_default: false })
    }

    pub fn remove_folder(&self, id: i64) -> Result<()> {
        if self.default_folder_id()? == Some(id) {
            self.set_default_folder(None)?;
        }
        self.conn.execute("DELETE FROM folders WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_folders(&self) -> Result<Vec<Folder>> {
        let mut st = self.conn.prepare(
            "SELECT f.id, f.path, (SELECT COUNT(*) FROM videos v WHERE v.folder_id = f.id)
             FROM folders f ORDER BY f.path COLLATE NOCASE",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Folder { id: r.get(0)?, path: r.get(1)?, video_count: r.get(2)?, is_default: false })
        })?;
        let mut folders: Vec<Folder> = rows.collect::<std::result::Result<_, _>>()?;
        let default = self.default_folder_id()?;
        for f in &mut folders {
            f.is_default = Some(f.id) == default;
        }
        Ok(folders)
    }

    /// The folder the library shows by default (None = all folders).
    pub fn default_folder_id(&self) -> Result<Option<i64>> {
        let id = self.get_setting("default_folder_id")?.and_then(|v| v.parse::<i64>().ok());
        match id {
            // Treat a stale id as "none".
            Some(i) if self.folder_path(i).is_ok() => Ok(Some(i)),
            _ => Ok(None),
        }
    }

    pub fn set_default_folder(&self, id: Option<i64>) -> Result<()> {
        if let Some(i) = id {
            self.folder_path(i)?; // must exist
        }
        self.set_setting("default_folder_id", &id.map(|i| i.to_string()).unwrap_or_default())
    }

    pub fn folder_path(&self, id: i64) -> Result<String> {
        self.conn
            .query_row("SELECT path FROM folders WHERE id = ?1", params![id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| Error::Msg(format!("folder {id} not found")))
    }

    pub fn folder_ids(&self) -> Result<Vec<i64>> {
        let mut st = self.conn.prepare("SELECT id FROM folders ORDER BY id")?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // --------------------------------------------------------------- scanning

    /// Scan a folder (filesystem walk + DB update). The walk does not touch the
    /// DB, so callers holding a lock may prefer `folder_path` + `scan_dir` +
    /// `apply_scan` to keep the lock short.
    pub fn scan_folder(&mut self, folder_id: i64) -> Result<ScanReport> {
        let path = self.folder_path(folder_id)?;
        let files = scan_folder_files(&path)?;
        self.apply_scan(folder_id, &files)
    }

    /// Apply the result of a filesystem scan: insert new videos, update changed
    /// ones and drop rows whose files are gone.
    pub fn apply_scan(&mut self, folder_id: i64, files: &[ScannedFile]) -> Result<ScanReport> {
        let tx = self.conn.transaction()?;
        let mut report = ScanReport { total: files.len(), ..Default::default() };
        let seen: HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
        let rules = crate::rules::load_enabled(&tx)?;
        {
            let mut exists = tx.prepare("SELECT 1 FROM videos WHERE path = ?1")?;
            let mut upsert = tx.prepare(
                "INSERT INTO videos(path, folder_id, title, size, modified, created)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(path) DO UPDATE SET size = excluded.size, modified = excluded.modified,
                                                 created = excluded.created,
                                                 -- a changed file needs its duration re-read
                                                 duration = CASE WHEN excluded.size != size OR excluded.modified != modified
                                                                 THEN NULL ELSE duration END,
                                                 thumb = CASE WHEN excluded.size != size OR excluded.modified != modified
                                                              THEN NULL ELSE thumb END",
            )?;
            for f in files {
                let known = exists.exists(params![f.path])?;
                upsert.execute(params![f.path, folder_id, f.title, f.size, f.modified, f.created])?;
                if !known {
                    report.added += 1;
                    // Automatic rules apply to newly added videos only, so tags the
                    // user removes by hand don't come back on every rescan.
                    let video_id = tx.last_insert_rowid();
                    if crate::rules::apply(&tx, &rules, video_id, &f.path)? {
                        report.rule_matches += 1;
                    }
                }
            }
        }
        let stale: Vec<i64> = {
            let mut st = tx.prepare("SELECT id, path FROM videos WHERE folder_id = ?1")?;
            let rows = st.query_map(params![folder_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            let mut v = Vec::new();
            for row in rows {
                let (id, p) = row?;
                if !seen.contains(p.as_str()) {
                    v.push(id);
                }
            }
            v
        };
        for id in &stale {
            tx.execute("DELETE FROM videos WHERE id = ?1", params![id])?;
        }
        report.removed = stale.len();
        tx.commit()?;
        Ok(report)
    }

    // ----------------------------------------------------------------- videos

    pub fn get_video(&self, id: i64) -> Result<Video> {
        let mut v = self.query_videos("v.id = ?", vec![Value::Integer(id)], None, SortBy::Default, false, None)?;
        v.pop().ok_or_else(|| Error::Msg(format!("video {id} not found")))
    }

    pub fn list_videos(&self, filter: &VideoFilter) -> Result<Vec<Video>> {
        let (where_sql, args, playlist) = self.filter_sql(filter);
        self.query_videos(&where_sql, args, playlist, filter.sort_by, filter.sort_desc, None)
    }

    /// One page of the filtered/sorted list plus the total number of matches.
    /// `page` is 0-based; `page_size` is clamped to at least 1.
    pub fn list_videos_page(&self, filter: &VideoFilter, page: usize, page_size: usize) -> Result<VideoPage> {
        let (where_sql, args, playlist) = self.filter_sql(filter);
        let join = if playlist.is_some() { "JOIN playlist_items pi ON pi.video_id = v.id AND pi.playlist_id = ?" } else { "" };
        let mut count_args: Vec<Value> = Vec::new();
        if let Some(pl) = playlist {
            count_args.push(Value::Integer(pl));
        }
        count_args.extend(args.iter().cloned());
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM videos v {join} WHERE {where_sql}"),
            params_from_iter(count_args),
            |r| r.get(0),
        )?;
        let page_size = page_size.max(1);
        // Past the end (e.g. after deletions): fall back to the last page.
        let last = (total as usize).saturating_sub(1) / page_size;
        let page = page.min(last);
        let items = self.query_videos(
            &where_sql,
            args,
            playlist,
            filter.sort_by,
            filter.sort_desc,
            Some((page_size as i64, (page * page_size) as i64)),
        )?;
        Ok(VideoPage { items, total: total as usize, page, page_size })
    }

    fn filter_sql(&self, filter: &VideoFilter) -> (String, Vec<Value>, Option<i64>) {
        let mut clauses: Vec<String> = Vec::new();
        let mut args: Vec<Value> = Vec::new();
        let mut join_playlist = None;

        if let Some(pl) = filter.playlist_id {
            join_playlist = Some(pl);
        }
        if let Some(s) = filter.search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let like = format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
            clauses.push("(v.title LIKE ? ESCAPE '\\' OR v.path LIKE ? ESCAPE '\\')".into());
            args.push(Value::Text(like.clone()));
            args.push(Value::Text(like));
        }
        if let Some(w) = filter.watched {
            clauses.push("v.watched = ?".into());
            args.push(Value::Integer(w as i64));
        }
        if let Some(f) = filter.folder_id {
            // Match by path prefix (not folder_id) so videos owned by an overlapping
            // parent folder still show up under a subfolder.
            match self.folder_path(f) {
                Ok(path) => {
                    let mut prefix = path;
                    if !prefix.ends_with(std::path::MAIN_SEPARATOR) {
                        prefix.push(std::path::MAIN_SEPARATOR);
                    }
                    clauses.push("substr(v.path, 1, ?) = ?".into());
                    args.push(Value::Integer(prefix.chars().count() as i64));
                    args.push(Value::Text(prefix));
                }
                Err(_) => clauses.push("0".into()),
            }
        }
        if !filter.tag_ids.is_empty() {
            let marks = vec!["?"; filter.tag_ids.len()].join(",");
            clauses.push(format!(
                "(SELECT COUNT(DISTINCT tag_id) FROM video_tags WHERE video_id = v.id AND tag_id IN ({marks})) = ?"
            ));
            let unique: HashSet<i64> = filter.tag_ids.iter().copied().collect();
            for t in &filter.tag_ids {
                args.push(Value::Integer(*t));
            }
            args.push(Value::Integer(unique.len() as i64));
        }
        let where_sql = if clauses.is_empty() { "1=1".to_string() } else { clauses.join(" AND ") };
        (where_sql, args, join_playlist)
    }

    fn query_videos(
        &self,
        where_sql: &str,
        args: Vec<Value>,
        playlist: Option<i64>,
        sort_by: SortBy,
        desc: bool,
        limit_offset: Option<(i64, i64)>,
    ) -> Result<Vec<Video>> {
        let mut all_args: Vec<Value> = Vec::new();
        let join = match playlist {
            Some(pl) => {
                all_args.push(Value::Integer(pl));
                "JOIN playlist_items pi ON pi.video_id = v.id AND pi.playlist_id = ?"
            }
            None => "",
        };
        let dir = if desc { "DESC" } else { "ASC" };
        // Only fixed strings are interpolated into the SQL (never user input).
        let order = match (sort_by, playlist) {
            (SortBy::Default, Some(_)) => format!("pi.position {dir}"),
            (SortBy::Default, None) | (SortBy::Title, _) => format!("v.title COLLATE NOCASE {dir}, v.id ASC"),
            (SortBy::Path, _) => format!("v.path COLLATE NOCASE {dir}, v.id ASC"),
            (SortBy::Created, _) => format!("v.created {dir}, v.id ASC"),
            (SortBy::Size, _) => format!("v.size {dir}, v.id ASC"),
            (SortBy::Rating, _) => format!("(v.rating = 0) ASC, v.rating {dir}, v.id ASC"),
            (SortBy::Duration, _) => format!(
                "(v.duration IS NULL OR v.duration <= 0) ASC, v.duration {dir}, v.id ASC"
            ),
        };
        all_args.extend(args);
        let limit = match limit_offset {
            Some((l, o)) => format!(" LIMIT {l} OFFSET {o}"),
            None => String::new(),
        };
        let sql = format!(
            "SELECT v.id, v.path, v.folder_id, v.title, v.size, v.modified, v.watched, v.created, v.rating, v.duration, v.thumb
             FROM videos v {join} WHERE {where_sql} ORDER BY {order}{limit}"
        );
        let mut st = self.conn.prepare(&sql)?;
        let mut videos: Vec<Video> = st
            .query_map(params_from_iter(all_args), |r| {
                Ok(Video {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    folder_id: r.get(2)?,
                    title: r.get(3)?,
                    size: r.get(4)?,
                    modified: r.get(5)?,
                    watched: r.get::<_, i64>(6)? != 0,
                    created: r.get(7)?,
                    rating: r.get::<_, i64>(8)? as f64 / 2.0,
                    duration: r.get(9)?,
                    thumb: r.get::<_, Option<i64>>(10)? == Some(1),
                    tags: Vec::new(),
                    playlist_ids: Vec::new(),
                })
            })?
            .collect::<std::result::Result<_, _>>()?;

        if !videos.is_empty() {
            let mut tags: HashMap<i64, Vec<VideoTag>> = HashMap::new();
            let mut st = self.conn.prepare(
                "SELECT vt.video_id, t.id, t.name, t.color FROM video_tags vt
                 JOIN tags t ON t.id = vt.tag_id ORDER BY t.name COLLATE NOCASE",
            )?;
            for row in st.query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, VideoTag { id: r.get(1)?, name: r.get(2)?, color: r.get(3)? }))
            })? {
                let (vid, tag) = row?;
                tags.entry(vid).or_default().push(tag);
            }
            let mut pls: HashMap<i64, Vec<i64>> = HashMap::new();
            let mut st = self.conn.prepare("SELECT video_id, playlist_id FROM playlist_items")?;
            for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
                let (vid, pl) = row?;
                pls.entry(vid).or_default().push(pl);
            }
            for v in &mut videos {
                v.tags = tags.remove(&v.id).unwrap_or_default();
                v.playlist_ids = pls.remove(&v.id).unwrap_or_default();
            }
        }
        Ok(videos)
    }

    pub fn set_watched(&self, video_id: i64, watched: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE videos SET watched = ?1 WHERE id = ?2",
            params![watched as i64, video_id],
        )?;
        Ok(())
    }

    /// Videos whose duration has not been read yet (oldest first).
    /// Mark every video matching `filter` (all pages) as watched, or, when they
    /// already all are, as unwatched. Returns the new watched state.
    pub fn toggle_watched_matching(&self, filter: &VideoFilter) -> Result<bool> {
        let videos = self.list_videos(filter)?;
        let watched = videos.iter().any(|v| !v.watched);
        let tx = self.conn.unchecked_transaction()?;
        for v in &videos {
            tx.execute("UPDATE videos SET watched = ?1 WHERE id = ?2", params![watched as i64, v.id])?;
        }
        tx.commit()?;
        Ok(watched)
    }

    pub fn videos_missing_duration(&self, limit: usize) -> Result<Vec<(i64, String)>> {
        let mut st = self
            .conn
            .prepare("SELECT id, path FROM videos WHERE duration IS NULL ORDER BY id LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn count_missing_duration(&self) -> Result<usize> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM videos WHERE duration IS NULL", [], |r| r.get::<_, i64>(0))? as usize)
    }

    /// Store a duration in seconds (0 = unreadable, so it is not retried forever).
    pub fn set_duration(&self, video_id: i64, seconds: f64) -> Result<()> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(Error::Msg(format!("invalid duration {seconds}")));
        }
        self.conn.execute("UPDATE videos SET duration = ?1 WHERE id = ?2", params![seconds, video_id])?;
        Ok(())
    }

    /// Videos without a thumbnail attempt yet (oldest first).
    pub fn videos_missing_thumb(&self, limit: usize) -> Result<Vec<(i64, String, Option<f64>)>> {
        let mut st = self
            .conn
            .prepare("SELECT id, path, duration FROM videos WHERE thumb IS NULL ORDER BY id LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn count_missing_thumb(&self) -> Result<usize> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM videos WHERE thumb IS NULL", [], |r| r.get::<_, i64>(0))? as usize)
    }

    /// Store a thumbnail (JPEG bytes), or record a failed attempt with `None`
    /// so it is not retried forever.
    pub fn set_thumb(&self, video_id: i64, jpeg: Option<&[u8]>) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        match jpeg {
            Some(data) => {
                tx.execute("INSERT OR REPLACE INTO thumbnails(video_id, data) VALUES (?1, ?2)", params![video_id, data])?;
            }
            None => {
                tx.execute("DELETE FROM thumbnails WHERE video_id = ?1", params![video_id])?;
            }
        }
        tx.execute("UPDATE videos SET thumb = ?1 WHERE id = ?2", params![jpeg.is_some() as i64, video_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_thumb(&self, video_id: i64) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row("SELECT data FROM thumbnails WHERE video_id = ?1", params![video_id], |r| r.get(0))
            .optional()?)
    }

    /// Set a star rating: 0 clears it, otherwise 0.5..=5.0 in steps of 0.5.
    pub fn set_rating(&self, video_id: i64, stars: f64) -> Result<()> {
        let halves = stars * 2.0;
        if !(0.0..=10.0).contains(&halves) || halves.fract() != 0.0 {
            return Err(Error::Msg(format!("invalid rating {stars}: use 0 to 5 in steps of 0.5")));
        }
        self.conn.execute(
            "UPDATE videos SET rating = ?1 WHERE id = ?2",
            params![halves as i64, video_id],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------- tags

    pub fn create_tag(&self, name: &str, color: &str) -> Result<Tag> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Msg("tag name cannot be empty".into()));
        }
        self.conn
            .execute("INSERT INTO tags(name, color) VALUES (?1, ?2)", params![name, color])
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
                    Error::Msg(format!("a tag named '{name}' already exists"))
                }
                other => other.into(),
            })?;
        Ok(Tag { id: self.conn.last_insert_rowid(), name: name.into(), color: color.into(), video_count: 0 })
    }

    pub fn update_tag(&self, id: i64, name: &str, color: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Msg("tag name cannot be empty".into()));
        }
        self.conn.execute(
            "UPDATE tags SET name = ?1, color = ?2 WHERE id = ?3",
            params![name, color, id],
        )?;
        Ok(())
    }

    pub fn delete_tag(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM tags WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut st = self.conn.prepare(
            "SELECT t.id, t.name, t.color, (SELECT COUNT(*) FROM video_tags vt WHERE vt.tag_id = t.id)
             FROM tags t ORDER BY t.name COLLATE NOCASE",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, video_count: r.get(3)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn set_video_tag(&self, video_id: i64, tag_id: i64, on: bool) -> Result<()> {
        if on {
            self.conn.execute(
                "INSERT OR IGNORE INTO video_tags(video_id, tag_id) VALUES (?1, ?2)",
                params![video_id, tag_id],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM video_tags WHERE video_id = ?1 AND tag_id = ?2",
                params![video_id, tag_id],
            )?;
        }
        Ok(())
    }

    // -------------------------------------------------------------- playlists

    pub fn create_playlist(&self, name: &str) -> Result<Playlist> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Msg("playlist name cannot be empty".into()));
        }
        self.conn
            .execute("INSERT INTO playlists(name) VALUES (?1)", params![name])
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
                    Error::Msg(format!("a playlist named '{name}' already exists"))
                }
                other => other.into(),
            })?;
        Ok(Playlist { id: self.conn.last_insert_rowid(), name: name.into(), video_count: 0 })
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Msg("playlist name cannot be empty".into()));
        }
        self.conn.execute("UPDATE playlists SET name = ?1 WHERE id = ?2", params![name, id])?;
        Ok(())
    }

    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_playlists(&self) -> Result<Vec<Playlist>> {
        let mut st = self.conn.prepare(
            "SELECT p.id, p.name, (SELECT COUNT(*) FROM playlist_items i WHERE i.playlist_id = p.id)
             FROM playlists p ORDER BY p.name COLLATE NOCASE",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Playlist { id: r.get(0)?, name: r.get(1)?, video_count: r.get(2)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Append a video at the end of a playlist (no-op if already present).
    pub fn add_to_playlist(&self, playlist_id: i64, video_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO playlist_items(playlist_id, video_id, position)
             VALUES (?1, ?2, COALESCE((SELECT MAX(position) + 1 FROM playlist_items WHERE playlist_id = ?1), 0))",
            params![playlist_id, video_id],
        )?;
        Ok(())
    }

    pub fn remove_from_playlist(&self, playlist_id: i64, video_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM playlist_items WHERE playlist_id = ?1 AND video_id = ?2",
            params![playlist_id, video_id],
        )?;
        Ok(())
    }

    /// Reorder: videos listed in `ordered_ids` take positions 0..n in that order;
    /// any playlist videos not listed keep their relative order after them.
    pub fn reorder_playlist(&mut self, playlist_id: i64, ordered_ids: &[i64]) -> Result<()> {
        let tx = self.conn.transaction()?;
        let current: Vec<i64> = {
            let mut st = tx.prepare(
                "SELECT video_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position",
            )?;
            let rows = st.query_map(params![playlist_id], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let in_playlist: HashSet<i64> = current.iter().copied().collect();
        let mut final_order: Vec<i64> = Vec::new();
        let mut used = HashSet::new();
        for id in ordered_ids {
            if in_playlist.contains(id) && used.insert(*id) {
                final_order.push(*id);
            }
        }
        for id in current {
            if used.insert(id) {
                final_order.push(id);
            }
        }
        for (pos, id) in final_order.iter().enumerate() {
            tx.execute(
                "UPDATE playlist_items SET position = ?1 WHERE playlist_id = ?2 AND video_id = ?3",
                params![pos as i64, playlist_id, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // --------------------------------------------------------------- settings

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let stored: Option<String> = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
            .optional()?;
        Ok(stored.or_else(|| {
            DEFAULT_SETTINGS.iter().find(|(k, _)| *k == key).map(|(_, v)| v.to_string())
        }))
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        if !DEFAULT_SETTINGS.iter().any(|(k, _)| *k == key) {
            return Err(Error::Msg(format!("unknown setting '{key}'")));
        }
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// All settings, defaults included.
    pub fn all_settings(&self) -> Result<HashMap<String, String>> {
        let mut map: HashMap<String, String> =
            DEFAULT_SETTINGS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let mut st = self.conn.prepare("SELECT key, value FROM settings")?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (k, v) = row?;
            map.insert(k, v);
        }
        Ok(map)
    }
}

/// Walk a folder; fails (instead of reporting "0 videos") if the root is missing,
/// so an unplugged drive never wipes the library.
fn scan_folder_files(path: &str) -> Result<Vec<ScannedFile>> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Err(Error::Msg(format!("folder '{path}' is not accessible")));
    }
    Ok(scan_dir(p))
}

pub fn scan_folder_files_pub(path: &str) -> Result<Vec<ScannedFile>> {
    scan_folder_files(path)
}

/// Windows `canonicalize` yields `\\?\C:\...`; strip it for nicer display/mpv usage.
fn strip_verbatim(p: &str) -> String {
    p.strip_prefix(r"\\?\").unwrap_or(p).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    fn setup() -> (tempfile::TempDir, Library, Folder) {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "a.mp4");
        touch(dir.path(), "sub/b.MKV");
        touch(dir.path(), "sub/deep/c.webm");
        touch(dir.path(), "notes.txt");
        let mut lib = Library::open_in_memory().unwrap();
        let folder = lib.add_folder(dir.path().to_str().unwrap()).unwrap();
        lib.scan_folder(folder.id).unwrap();
        (dir, lib, folder)
    }

    #[test]
    fn scan_finds_recursive_videos_only() {
        let (_d, lib, _f) = setup();
        let vids = lib.list_videos(&VideoFilter::default()).unwrap();
        let titles: Vec<_> = vids.iter().map(|v| v.title.as_str()).collect();
        assert_eq!(titles, vec!["a", "b", "c"]);
        assert_eq!(lib.list_folders().unwrap()[0].video_count, 3);
    }

    #[test]
    fn rescan_adds_and_removes() {
        let (d, mut lib, f) = setup();
        fs::remove_file(d.path().join("a.mp4")).unwrap();
        touch(d.path(), "new.avi");
        let r = lib.scan_folder(f.id).unwrap();
        assert_eq!((r.added, r.removed, r.total), (1, 1, 3));
        let titles: Vec<_> = lib.list_videos(&VideoFilter::default()).unwrap().into_iter().map(|v| v.title).collect();
        assert_eq!(titles, vec!["b", "c", "new"]);
    }

    #[test]
    fn missing_root_does_not_wipe_library() {
        let (d, mut lib, f) = setup();
        let path = d.path().to_path_buf();
        drop(d); // deletes the directory
        assert!(!path.exists());
        assert!(lib.scan_folder(f.id).is_err());
        assert_eq!(lib.list_videos(&VideoFilter::default()).unwrap().len(), 3);
    }

    #[test]
    fn toggle_watched_matching_flips_all_then_back() {
        let (_d, lib, _f) = setup();
        let f = VideoFilter::default();
        let first = lib.list_videos(&f).unwrap()[0].id;
        lib.set_watched(first, true).unwrap(); // mixed state -> everything becomes watched
        assert!(lib.toggle_watched_matching(&f).unwrap());
        assert!(lib.list_videos(&f).unwrap().iter().all(|v| v.watched));
        assert!(!lib.toggle_watched_matching(&f).unwrap());
        assert!(lib.list_videos(&f).unwrap().iter().all(|v| !v.watched));
    }

    #[test]
    fn pages_slice_sorted_results_and_report_total() {
        let (_d, lib, _f) = setup(); // 3 videos
        let f = VideoFilter { sort_by: SortBy::Title, ..Default::default() };
        let all: Vec<_> = lib.list_videos(&f).unwrap().into_iter().map(|v| v.id).collect();
        let p0 = lib.list_videos_page(&f, 0, 2).unwrap();
        let p1 = lib.list_videos_page(&f, 1, 2).unwrap();
        assert_eq!((p0.total, p0.items.len(), p1.items.len()), (3, 2, 1));
        let paged: Vec<_> = p0.items.iter().chain(&p1.items).map(|v| v.id).collect();
        assert_eq!(paged, all);
        // Out-of-range pages clamp to the last one.
        let far = lib.list_videos_page(&f, 99, 2).unwrap();
        assert_eq!((far.page, far.items.len()), (1, 1));
        // Filters apply to the total.
        let none = lib.list_videos_page(&VideoFilter { search: Some("zzz".into()), ..Default::default() }, 0, 2).unwrap();
        assert_eq!((none.total, none.items.len()), (0, 0));
    }

    #[test]
    fn add_folder_rejects_non_directory() {
        let lib = Library::open_in_memory().unwrap();
        assert!(lib.add_folder("/definitely/not/here").is_err());
    }

    #[test]
    fn removing_folder_removes_videos() {
        let (_d, lib, f) = setup();
        lib.remove_folder(f.id).unwrap();
        assert!(lib.list_videos(&VideoFilter::default()).unwrap().is_empty());
    }

    #[test]
    fn watched_and_filter() {
        let (_d, lib, _f) = setup();
        let vids = lib.list_videos(&VideoFilter::default()).unwrap();
        assert!(vids.iter().all(|v| !v.watched));
        lib.set_watched(vids[0].id, true).unwrap();
        let w = lib.list_videos(&VideoFilter { watched: Some(true), ..Default::default() }).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].id, vids[0].id);
        let u = lib.list_videos(&VideoFilter { watched: Some(false), ..Default::default() }).unwrap();
        assert_eq!(u.len(), 2);
        lib.set_watched(vids[0].id, false).unwrap();
        assert!(lib.get_video(vids[0].id).unwrap().watched == false);
    }

    #[test]
    fn search_filter_escapes_wildcards() {
        let (_d, lib, _f) = setup();
        let r = lib.list_videos(&VideoFilter { search: Some("b".into()), ..Default::default() }).unwrap();
        assert!(r.iter().any(|v| v.title == "b"));
        let r = lib.list_videos(&VideoFilter { search: Some("%".into()), ..Default::default() }).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn tags_many_per_video_and_and_filter() {
        let (_d, lib, _f) = setup();
        let vids = lib.list_videos(&VideoFilter::default()).unwrap();
        let t1 = lib.create_tag("Comedy", "#fff").unwrap();
        let t2 = lib.create_tag("2020", "#000").unwrap();
        assert!(lib.create_tag("comedy", "#fff").is_err(), "names are case-insensitive unique");
        lib.set_video_tag(vids[0].id, t1.id, true).unwrap();
        lib.set_video_tag(vids[0].id, t2.id, true).unwrap();
        lib.set_video_tag(vids[1].id, t1.id, true).unwrap();
        lib.set_video_tag(vids[1].id, t1.id, true).unwrap(); // idempotent

        let one = lib.list_videos(&VideoFilter { tag_ids: vec![t1.id], ..Default::default() }).unwrap();
        assert_eq!(one.len(), 2);
        let both = lib.list_videos(&VideoFilter { tag_ids: vec![t1.id, t2.id], ..Default::default() }).unwrap();
        assert_eq!(both.len(), 1);
        assert_eq!(both[0].tags.len(), 2);

        let tags = lib.list_tags().unwrap();
        assert_eq!(tags.iter().find(|t| t.id == t1.id).unwrap().video_count, 2);

        lib.set_video_tag(vids[0].id, t1.id, false).unwrap();
        assert_eq!(lib.get_video(vids[0].id).unwrap().tags.len(), 1);
        lib.delete_tag(t2.id).unwrap();
        assert!(lib.get_video(vids[0].id).unwrap().tags.is_empty());
    }

    #[test]
    fn playlists_ordered_and_reorderable() {
        let (_d, mut lib, _f) = setup();
        let vids = lib.list_videos(&VideoFilter::default()).unwrap();
        let (a, b, c) = (vids[0].id, vids[1].id, vids[2].id);
        let pl = lib.create_playlist("Night").unwrap();
        lib.add_to_playlist(pl.id, c).unwrap();
        lib.add_to_playlist(pl.id, a).unwrap();
        lib.add_to_playlist(pl.id, b).unwrap();
        lib.add_to_playlist(pl.id, b).unwrap(); // duplicate ignored
        let f = VideoFilter { playlist_id: Some(pl.id), ..Default::default() };
        let ids = |lib: &Library| lib.list_videos(&f).unwrap().into_iter().map(|v| v.id).collect::<Vec<_>>();
        assert_eq!(ids(&lib), vec![c, a, b]);
        lib.reorder_playlist(pl.id, &[b, c]).unwrap();
        assert_eq!(ids(&lib), vec![b, c, a]);
        lib.remove_from_playlist(pl.id, c).unwrap();
        assert_eq!(ids(&lib), vec![b, a]);
        assert_eq!(lib.list_playlists().unwrap()[0].video_count, 2);
        assert_eq!(lib.get_video(a).unwrap().playlist_ids, vec![pl.id]);
        lib.delete_playlist(pl.id).unwrap();
        assert!(lib.get_video(a).unwrap().playlist_ids.is_empty());
    }

    #[test]
    fn settings_defaults_and_overrides() {
        let lib = Library::open_in_memory().unwrap();
        assert_eq!(lib.get_setting("volume_max").unwrap().as_deref(), Some("300"));
        lib.set_setting("volume_max", "200").unwrap();
        assert_eq!(lib.get_setting("volume_max").unwrap().as_deref(), Some("200"));
        assert_eq!(lib.all_settings().unwrap()["volume_max"], "200");
        assert_eq!(lib.all_settings().unwrap()["primary_color"], "#d4a017");
        assert!(lib.set_setting("bogus", "1").is_err());
    }
}

#[cfg(test)]
mod folder_tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_folder_lifecycle_and_subfolder_filter() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(dir.path().join("root.mp4"), b"x").unwrap();
        fs::write(sub.join("inner.mp4"), b"x").unwrap();
        let mut lib = Library::open_in_memory().unwrap();
        let parent = lib.add_folder(dir.path().to_str().unwrap()).unwrap();
        let child = lib.add_folder(sub.to_str().unwrap()).unwrap();
        lib.scan_folder(parent.id).unwrap(); // parent owns both videos
        lib.scan_folder(child.id).unwrap();

        assert_eq!(lib.default_folder_id().unwrap(), None);
        lib.set_default_folder(Some(child.id)).unwrap();
        assert_eq!(lib.default_folder_id().unwrap(), Some(child.id));
        let flags: Vec<_> = lib.list_folders().unwrap().into_iter().map(|f| (f.id, f.is_default)).collect();
        assert!(flags.contains(&(child.id, true)) && flags.contains(&(parent.id, false)));

        // The subfolder filter finds the video even though the parent folder owns it.
        let v = lib.list_videos(&VideoFilter { folder_id: Some(child.id), ..Default::default() }).unwrap();
        assert_eq!(v.iter().map(|v| v.title.as_str()).collect::<Vec<_>>(), vec!["inner"]);
        let all = lib.list_videos(&VideoFilter { folder_id: Some(parent.id), ..Default::default() }).unwrap();
        assert_eq!(all.len(), 2);

        // Removing the default folder clears the default.
        lib.remove_folder(child.id).unwrap();
        assert_eq!(lib.default_folder_id().unwrap(), None);
        assert!(lib.set_default_folder(Some(9999)).is_err());
    }
}

#[cfg(test)]
mod rating_sort_tests {
    use super::*;
    use std::fs;

    fn lib_with(files: &[(&str, usize)]) -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        for (name, size) in files {
            fs::write(dir.path().join(name), vec![b'x'; *size]).unwrap();
        }
        let mut lib = Library::open_in_memory().unwrap();
        let f = lib.add_folder(dir.path().to_str().unwrap()).unwrap();
        lib.scan_folder(f.id).unwrap();
        (dir, lib)
    }

    fn titles(lib: &Library, sort_by: SortBy, desc: bool) -> Vec<String> {
        lib.list_videos(&VideoFilter { sort_by, sort_desc: desc, ..Default::default() })
            .unwrap()
            .into_iter()
            .map(|v| v.title)
            .collect()
    }

    #[test]
    fn sorts_by_size_path_title_and_direction() {
        let (_d, lib) = lib_with(&[("b.mp4", 30), ("a.mp4", 20), ("c.mp4", 10)]);
        assert_eq!(titles(&lib, SortBy::Size, false), vec!["c", "a", "b"]);
        assert_eq!(titles(&lib, SortBy::Size, true), vec!["b", "a", "c"]);
        assert_eq!(titles(&lib, SortBy::Path, false), vec!["a", "b", "c"]);
        assert_eq!(titles(&lib, SortBy::Path, true), vec!["c", "b", "a"]);
        assert_eq!(titles(&lib, SortBy::Title, true), vec!["c", "b", "a"]);
        assert_eq!(titles(&lib, SortBy::Default, false), vec!["a", "b", "c"]);
    }

    #[test]
    fn sorts_by_created_and_stores_it() {
        let (_d, lib) = lib_with(&[("a.mp4", 1), ("b.mp4", 1)]);
        let vids = lib.list_videos(&VideoFilter::default()).unwrap();
        assert!(vids.iter().all(|v| v.created > 0));
        // Force distinct creation times and check both directions.
        lib.conn.execute("UPDATE videos SET created = 100 WHERE title = 'b'", []).unwrap();
        lib.conn.execute("UPDATE videos SET created = 200 WHERE title = 'a'", []).unwrap();
        assert_eq!(titles(&lib, SortBy::Created, false), vec!["b", "a"]);
        assert_eq!(titles(&lib, SortBy::Created, true), vec!["a", "b"]);
    }

    #[test]
    fn default_sort_in_playlist_is_playlist_order_but_explicit_sort_wins() {
        let (_d, lib) = lib_with(&[("a.mp4", 10), ("b.mp4", 20)]);
        let v = lib.list_videos(&VideoFilter::default()).unwrap();
        let pl = lib.create_playlist("p").unwrap();
        lib.add_to_playlist(pl.id, v[1].id).unwrap(); // b first
        lib.add_to_playlist(pl.id, v[0].id).unwrap();
        let f = |sort_by, sort_desc| VideoFilter { playlist_id: Some(pl.id), sort_by, sort_desc, ..Default::default() };
        let t = |f: VideoFilter| lib.list_videos(&f).unwrap().into_iter().map(|v| v.title).collect::<Vec<_>>();
        assert_eq!(t(f(SortBy::Default, false)), vec!["b", "a"]);
        assert_eq!(t(f(SortBy::Size, false)), vec!["a", "b"]);
    }

    #[test]
    fn sorts_by_rating_with_unrated_last() {
        let (_d, lib) = lib_with(&[("a.mp4", 1), ("b.mp4", 1), ("c.mp4", 1), ("d.mp4", 1)]);
        let ids: Vec<i64> = lib.list_videos(&VideoFilter::default()).unwrap().iter().map(|v| v.id).collect();
        lib.set_rating(ids[0], 2.5).unwrap(); // a
        lib.set_rating(ids[1], 5.0).unwrap(); // b
        lib.set_rating(ids[2], 0.5).unwrap(); // c
        // d stays unrated
        assert_eq!(titles(&lib, SortBy::Rating, true), vec!["b", "a", "c", "d"]);
        assert_eq!(titles(&lib, SortBy::Rating, false), vec!["c", "a", "b", "d"]);
    }

    #[test]
    fn rating_validation_and_roundtrip() {
        let (_d, lib) = lib_with(&[("a.mp4", 1)]);
        let id = lib.list_videos(&VideoFilter::default()).unwrap()[0].id;
        assert_eq!(lib.get_video(id).unwrap().rating, 0.0);
        for good in [0.5, 1.0, 3.5, 5.0] {
            lib.set_rating(id, good).unwrap();
            assert_eq!(lib.get_video(id).unwrap().rating, good);
        }
        for bad in [-0.5, 5.5, 3.3, f64::NAN] {
            assert!(lib.set_rating(id, bad).is_err(), "{bad} should be rejected");
        }
        assert_eq!(lib.get_video(id).unwrap().rating, 5.0);
        lib.set_rating(id, 0.0).unwrap();
        assert_eq!(lib.get_video(id).unwrap().rating, 0.0);
    }

    #[test]
    fn old_databases_are_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.sqlite3");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE folders (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE);
                 CREATE TABLE videos (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE,
                   folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
                   title TEXT NOT NULL, size INTEGER NOT NULL DEFAULT 0, modified INTEGER NOT NULL DEFAULT 0,
                   watched INTEGER NOT NULL DEFAULT 0, added_at INTEGER NOT NULL DEFAULT 0);
                 INSERT INTO folders(id, path) VALUES (1, '/x');
                 INSERT INTO videos(path, folder_id, title) VALUES ('/x/a.mp4', 1, 'a');",
            )
            .unwrap();
        }
        let lib = Library::open(&path).unwrap();
        let v = lib.list_videos(&VideoFilter::default()).unwrap();
        assert_eq!((v.len(), v[0].rating, v[0].created), (1, 0.0, 0));
        lib.set_rating(v[0].id, 4.5).unwrap();
        drop(lib);
        // Reopening is idempotent.
        assert_eq!(Library::open(&path).unwrap().list_videos(&VideoFilter::default()).unwrap()[0].rating, 4.5);
    }
}

#[cfg(test)]
mod duration_tests {
    use super::*;
    use std::fs;

    #[test]
    fn duration_indexing_queue_sorting_and_invalidation() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["a.mp4", "b.mp4", "c.mp4"] {
            fs::write(dir.path().join(n), b"x").unwrap();
        }
        let mut lib = Library::open_in_memory().unwrap();
        let f = lib.add_folder(dir.path().to_str().unwrap()).unwrap();
        lib.scan_folder(f.id).unwrap();
        assert_eq!(lib.count_missing_duration().unwrap(), 3);

        let vids = lib.list_videos(&VideoFilter::default()).unwrap();
        assert!(vids.iter().all(|v| v.duration.is_none()));
        let (a, b, c) = (vids[0].id, vids[1].id, vids[2].id);
        lib.set_duration(a, 300.0).unwrap();
        lib.set_duration(b, 0.0).unwrap(); // unreadable: counted as done, shown as unknown
        assert!(lib.set_duration(c, -1.0).is_err());
        let missing = lib.videos_missing_duration(10).unwrap();
        assert_eq!(missing.iter().map(|m| m.0).collect::<Vec<_>>(), vec![c]);

        lib.set_duration(c, 60.0).unwrap();
        let order = |desc| {
            lib.list_videos(&VideoFilter { sort_by: SortBy::Duration, sort_desc: desc, ..Default::default() })
                .unwrap()
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(order(false), vec![c, a, b]); // unknown (b) last
        assert_eq!(order(true), vec![a, c, b]); // ...in both directions

        // Rescanning an unchanged file keeps the duration; a changed file resets it.
        lib.scan_folder(f.id).unwrap();
        assert_eq!(lib.get_video(a).unwrap().duration, Some(300.0));
        fs::write(dir.path().join("a.mp4"), b"much longer content now").unwrap();
        lib.scan_folder(f.id).unwrap();
        assert_eq!(lib.get_video(a).unwrap().duration, None);
        assert_eq!(lib.get_video(c).unwrap().duration, Some(60.0));
    }
}

#[cfg(test)]
mod thumb_tests {
    use super::*;

    #[test]
    fn thumbnails_are_stored_replaced_and_deleted_with_the_video() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mp4"), b"x").unwrap();
        let mut lib = Library::open_in_memory().unwrap();
        let folder = lib.add_folder(dir.path().to_str().unwrap()).unwrap();
        lib.scan_folder(folder.id).unwrap();
        let id = lib.list_videos(&VideoFilter::default()).unwrap()[0].id;
        assert_eq!(lib.count_missing_thumb().unwrap(), 1);

        lib.set_thumb(id, Some(&[1, 2, 3])).unwrap();
        assert_eq!(lib.get_thumb(id).unwrap(), Some(vec![1, 2, 3]));
        assert!(lib.get_video(id).unwrap().thumb);
        assert_eq!(lib.count_missing_thumb().unwrap(), 0);

        lib.set_thumb(id, None).unwrap(); // failed attempt drops the image but is not retried
        assert_eq!(lib.get_thumb(id).unwrap(), None);
        assert!(!lib.get_video(id).unwrap().thumb);
        assert_eq!(lib.count_missing_thumb().unwrap(), 0);

        lib.set_thumb(id, Some(&[9])).unwrap();
        std::fs::remove_file(dir.path().join("a.mp4")).unwrap();
        lib.scan_folder(folder.id).unwrap(); // video vanished -> thumbnail goes with it
        let n: i64 = lib.conn.query_row("SELECT COUNT(*) FROM thumbnails", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }
}
