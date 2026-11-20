//! Renaming and moving video files from the app.
//!
//! The database row keeps its id (so tags, playlists, rating, watched flag and thumbnail survive)
//! and its `path`, `folder_id` and `title` follow the file. `folder_id` matters: a rescan drops any
//! video of a folder whose path it no longer finds there.
//!
//! The caller drives three steps so that no lock is held during a slow copy:
//! `plan_*` (read-only) -> `apply_plan` (same-drive rename + DB update, instant) and, only when that
//! says the file must cross drives, `copy_across` (slow, lock-free) -> `commit_plan` (DB update).

use crate::db::strip_verbatim;
use crate::error::{Error, Result};
use crate::Library;
use rusqlite::{params, OptionalExtension};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MovePlan {
    pub video_id: i64,
    pub from: PathBuf,
    pub to: PathBuf,
    pub folder_id: i64,
}

impl MovePlan {
    fn title(&self) -> String {
        self.to.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
    "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Check a file name without its extension; returns it trimmed. Rules are the strictest of
/// Windows and Linux so a library can be used on both.
pub fn validate_stem(stem: &str) -> Result<String> {
    let stem = stem.trim();
    let bad = |m: &str| Err(Error::Msg(m.to_string()));
    if stem.is_empty() {
        return bad("the name cannot be empty");
    }
    if stem == "." || stem == ".." {
        return bad("that is not a valid file name");
    }
    if let Some(c) = stem.chars().find(|c| c.is_control() || "/\\<>:\"|?*".contains(*c)) {
        return Err(Error::Msg(format!("the name cannot contain '{}'", c.escape_default())));
    }
    if stem.ends_with('.') {
        return bad("the name cannot end with a dot");
    }
    // 255 bytes is the usual limit for a file name; leave room for the extension.
    if stem.len() > 240 {
        return bad("the name is too long");
    }
    let base = stem.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    if WINDOWS_RESERVED.contains(&base.as_str()) {
        return Err(Error::Msg(format!("'{base}' is a reserved name on Windows")));
    }
    Ok(stem.to_string())
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

fn canonical(p: &Path) -> PathBuf {
    let c = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    PathBuf::from(strip_verbatim(&c.to_string_lossy()))
}

impl Library {
    fn video_location(&self, id: i64) -> Result<(PathBuf, i64)> {
        self.conn
            .query_row("SELECT path, folder_id FROM videos WHERE id = ?1", params![id], |r| {
                Ok((PathBuf::from(r.get::<_, String>(0)?), r.get::<_, i64>(1)?))
            })
            .optional()?
            .ok_or_else(|| Error::Msg(format!("video {id} not found")))
    }

    /// The target must be free on disk and in the library (the one exception: a case-only rename
    /// on a case-insensitive drive, where the "other" file is the video itself).
    fn check_target_free(&self, video_id: i64, from: &Path, to: &Path) -> Result<()> {
        if to.exists() && !same_file(from, to) {
            return Err(Error::Msg(format!("'{}' already exists", to.display())));
        }
        let taken: bool = self
            .conn
            .prepare("SELECT 1 FROM videos WHERE path = ?1 AND id != ?2")?
            .exists(params![to.to_string_lossy(), video_id])?;
        if taken {
            return Err(Error::Msg(format!("'{}' is already in the library", to.display())));
        }
        Ok(())
    }

    /// Rename a video's file (new name without extension; the extension is kept).
    pub fn plan_rename(&self, video_id: i64, new_stem: &str) -> Result<MovePlan> {
        let stem = validate_stem(new_stem)?;
        let (from, folder_id) = self.video_location(video_id)?;
        let mut name = stem;
        if let Some(ext) = from.extension().and_then(OsStr::to_str) {
            name = format!("{name}.{ext}");
        }
        let to = from.with_file_name(name);
        if to == from {
            return Err(Error::Msg("the name is unchanged".into()));
        }
        self.check_target_free(video_id, &from, &to)?;
        Ok(MovePlan { video_id, from, to, folder_id })
    }

    /// Move a video's file into `dest_dir`, which must be one of the library folders or inside one
    /// (anywhere else the next scan would forget the video and its tags, playlists and rating).
    pub fn plan_move(&self, video_id: i64, dest_dir: &str) -> Result<MovePlan> {
        let (from, _) = self.video_location(video_id)?;
        let dest = Path::new(dest_dir);
        if !dest.is_dir() {
            return Err(Error::Msg(format!("'{dest_dir}' is not a folder")));
        }
        let dest_c = canonical(dest);
        // The library folder holding `dest`: the deepest one wins (folders may be nested).
        let mut best: Option<(usize, i64, PathBuf)> = None;
        for id in self.folder_ids()? {
            let folder = PathBuf::from(self.folder_path(id)?);
            // Component-wise: /a/b must not claim /a/bc.
            if let Ok(rel) = dest_c.strip_prefix(canonical(&folder)) {
                let depth = folder.components().count();
                if best.as_ref().map_or(true, |(d, _, _)| depth > *d) {
                    best = Some((depth, id, folder.join(rel)));
                }
            }
        }
        let (_, folder_id, target_dir) =
            best.ok_or_else(|| Error::Msg("choose a folder that is part of the library (a library folder or one inside it)".to_string()))?;
        let file_name = from.file_name().ok_or_else(|| Error::Msg("video has no file name".into()))?;
        let to = target_dir.join(file_name);
        if to == from || same_file(&from, &to) {
            return Err(Error::Msg("the video is already in that folder".into()));
        }
        self.check_target_free(video_id, &from, &to)?;
        Ok(MovePlan { video_id, from, to, folder_id })
    }

    /// Point the video's row at its new location.
    pub fn commit_plan(&self, plan: &MovePlan) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE videos SET path = ?1, folder_id = ?2, title = ?3 WHERE id = ?4",
            params![plan.to.to_string_lossy(), plan.folder_id, plan.title(), plan.video_id],
        )?;
        if n == 0 {
            return Err(Error::Msg("the video is no longer in the library".into()));
        }
        Ok(())
    }

    /// Rename the file and update the row. Returns false (nothing changed) when the file is on
    /// another drive than its destination: use `copy_across` + `commit_plan` then.
    /// If the row cannot be updated the file goes back where it was.
    pub fn apply_plan(&self, plan: &MovePlan) -> Result<bool> {
        match std::fs::rename(&plan.from, &plan.to) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::CrossesDevices => return Ok(false),
            Err(e) => return Err(e.into()),
        }
        if let Err(e) = self.commit_plan(plan) {
            let _ = std::fs::rename(&plan.to, &plan.from);
            return Err(e);
        }
        Ok(true)
    }
}

/// Cross-drive move: copy, check the size, then delete the original. Never overwrites.
pub fn copy_across(plan: &MovePlan) -> Result<()> {
    if plan.to.exists() {
        return Err(Error::Msg(format!("'{}' already exists", plan.to.display())));
    }
    let copied = std::fs::copy(&plan.from, &plan.to);
    let ok = match &copied {
        Ok(n) => std::fs::metadata(&plan.from).map(|m| m.len() == *n).unwrap_or(false),
        Err(_) => false,
    };
    if !ok {
        let _ = std::fs::remove_file(&plan.to); // never leave a partial copy behind
        return Err(match copied {
            Err(e) => e.into(),
            Ok(_) => Error::Msg("the file changed while it was being copied".into()),
        });
    }
    std::fs::remove_file(&plan.from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::VideoFilter;
    use std::fs;

    fn setup() -> (tempfile::TempDir, Library, i64, i64) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("lib/sub")).unwrap();
        fs::create_dir_all(dir.path().join("lib2")).unwrap();
        fs::create_dir_all(dir.path().join("outside")).unwrap();
        fs::write(dir.path().join("lib/a.mp4"), b"aaa").unwrap();
        fs::write(dir.path().join("lib/b.mkv"), b"bbb").unwrap();
        let mut lib = Library::open_in_memory().unwrap();
        let f1 = lib.add_folder(dir.path().join("lib").to_str().unwrap()).unwrap();
        let f2 = lib.add_folder(dir.path().join("lib2").to_str().unwrap()).unwrap();
        lib.scan_folder(f1.id).unwrap();
        (dir, lib, f1.id, f2.id)
    }

    fn id_of(lib: &Library, title: &str) -> i64 {
        lib.list_videos(&VideoFilter::default()).unwrap().into_iter().find(|v| v.title == title).unwrap().id
    }

    #[test]
    fn stems_are_validated() {
        for ok in ["movie", "  trimmed  ", "a.b", "été 2", "CONSOLE"] {
            assert!(validate_stem(ok).is_ok(), "{ok}");
        }
        for bad in ["", "  ", ".", "..", "a/b", "a\\b", "a:b", "a?", "x.", "con", "NUL.txt", "lpt1", "a\nb"] {
            assert!(validate_stem(bad).is_err(), "{bad}");
        }
        assert_eq!(validate_stem("  x ").unwrap(), "x");
    }

    #[test]
    fn rename_keeps_extension_and_all_metadata_through_a_rescan() {
        let (d, mut lib, f1, _) = setup();
        let id = id_of(&lib, "a");
        let tag = lib.create_tag("t", "#fff").unwrap();
        lib.set_video_tag(id, tag.id, true).unwrap();
        lib.set_rating(id, 4.5).unwrap();

        let plan = lib.plan_rename(id, "renamed").unwrap();
        assert!(lib.apply_plan(&plan).unwrap());
        assert!(d.path().join("lib/renamed.mp4").exists() && !d.path().join("lib/a.mp4").exists());

        lib.scan_folder(f1).unwrap();
        let v = lib.get_video(id).unwrap();
        assert_eq!((v.title.as_str(), v.rating, v.tags.len()), ("renamed", 4.5, 1));
        assert!(v.path.ends_with("renamed.mp4"));
        assert_eq!(lib.list_videos(&VideoFilter::default()).unwrap().len(), 2);
    }

    #[test]
    fn rename_refuses_collisions_and_noops() {
        let (_d, lib, _, _) = setup();
        let id = id_of(&lib, "a");
        assert!(lib.plan_rename(id, "a").is_err()); // unchanged
        fs::write(Path::new(&lib.get_video(id).unwrap().path).with_file_name("taken.mp4"), b"x").unwrap();
        let err = lib.plan_rename(id, "taken").unwrap_err().to_string();
        assert!(err.contains("already exists"), "{err}");
        assert!(lib.plan_rename(id, "bad/name").is_err());
    }

    #[test]
    fn rename_never_overwrites_a_library_entry_whose_file_is_gone() {
        let (d, lib, _, _) = setup();
        let (a, b) = (id_of(&lib, "a"), id_of(&lib, "b"));
        let _ = b;
        fs::write(d.path().join("lib/b.mp4"), b"x").unwrap();
        lib.conn.execute("UPDATE videos SET path = ?1 WHERE id = ?2", params![d.path().join("lib/ghost.mp4").to_string_lossy(), b]).unwrap();
        // "ghost.mp4" is not on disk but the library already owns that path.
        assert!(lib.plan_rename(a, "ghost").unwrap_err().to_string().contains("already in the library"));
    }

    #[test]
    fn move_into_another_library_folder_updates_folder_id_and_survives_rescan() {
        let (d, mut lib, f1, f2) = setup();
        let id = id_of(&lib, "b");
        let tag = lib.create_tag("t", "#fff").unwrap();
        lib.set_video_tag(id, tag.id, true).unwrap();

        let plan = lib.plan_move(id, d.path().join("lib2").to_str().unwrap()).unwrap();
        assert_eq!(plan.folder_id, f2);
        assert!(lib.apply_plan(&plan).unwrap());
        assert!(d.path().join("lib2/b.mkv").exists() && !d.path().join("lib/b.mkv").exists());

        // Rescanning both folders must keep the row (and its tag), not drop and re-add it.
        lib.scan_folder(f1).unwrap();
        lib.scan_folder(f2).unwrap();
        let v = lib.get_video(id).unwrap();
        assert_eq!((v.folder_id, v.tags.len()), (f2, 1));
        assert_eq!(lib.list_videos(&VideoFilter::default()).unwrap().len(), 2);
    }

    #[test]
    fn move_into_a_subfolder_keeps_the_owning_folder() {
        let (d, mut lib, f1, _) = setup();
        let id = id_of(&lib, "a");
        let plan = lib.plan_move(id, d.path().join("lib/sub").to_str().unwrap()).unwrap();
        assert_eq!(plan.folder_id, f1);
        assert!(lib.apply_plan(&plan).unwrap());
        lib.scan_folder(f1).unwrap();
        assert!(lib.get_video(id).unwrap().path.contains("sub"));
    }

    #[test]
    fn move_outside_the_library_or_onto_itself_or_over_a_file_is_refused() {
        let (d, lib, _, _) = setup();
        let id = id_of(&lib, "a");
        assert!(lib.plan_move(id, d.path().join("outside").to_str().unwrap()).is_err());
        assert!(lib.plan_move(id, d.path().join("lib").to_str().unwrap()).unwrap_err().to_string().contains("already in that folder"));
        assert!(lib.plan_move(id, d.path().join("missing").to_str().unwrap()).is_err());
        fs::write(d.path().join("lib2/a.mp4"), b"other").unwrap();
        assert!(lib.plan_move(id, d.path().join("lib2").to_str().unwrap()).unwrap_err().to_string().contains("already exists"));
        assert_eq!(fs::read(d.path().join("lib2/a.mp4")).unwrap(), b"other");
    }

    #[test]
    fn a_sibling_directory_with_a_shared_prefix_is_not_inside_the_folder() {
        let (d, lib, _, _) = setup();
        fs::create_dir_all(d.path().join("lib-extra")).unwrap();
        let id = id_of(&lib, "a");
        assert!(lib.plan_move(id, d.path().join("lib-extra").to_str().unwrap()).is_err());
    }

    #[test]
    fn copy_across_moves_the_bytes_and_never_overwrites() {
        let (d, lib, _, f2) = setup();
        let id = id_of(&lib, "a");
        let plan = lib.plan_move(id, d.path().join("lib2").to_str().unwrap()).unwrap();
        copy_across(&plan).unwrap();
        lib.commit_plan(&plan).unwrap();
        assert_eq!(fs::read(d.path().join("lib2/a.mp4")).unwrap(), b"aaa");
        assert!(!d.path().join("lib/a.mp4").exists());
        assert_eq!(lib.get_video(id).unwrap().folder_id, f2);
        // Source is gone now, so a second copy fails without touching the target.
        assert!(copy_across(&plan).is_err());
        assert_eq!(fs::read(d.path().join("lib2/a.mp4")).unwrap(), b"aaa");
    }
}
