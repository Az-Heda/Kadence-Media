//! Export / import of tag, playlist and automatic-rule *definitions*.
//!
//! Videos are never part of the file: no paths, no tag/playlist membership, no ordering.
//! Rules refer to tags and playlists by name (ids differ between libraries). Import is additive
//! and idempotent: anything that already exists is skipped, so importing a file twice changes nothing.

use crate::error::{Error, Result};
use crate::Library;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "kadence-media-export";
pub const VERSION: u32 = 1;
/// Color given to tags that an imported rule needs but the file does not define.
const DEFAULT_TAG_COLOR: &str = "#d4a017";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTag {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPlaylist {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRule {
    pub name: String,
    pub pattern: String,
    #[serde(default = "yes")]
    pub case_insensitive: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub hide: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub playlists: Vec<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFile {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub tags: Vec<ExportTag>,
    #[serde(default)]
    pub playlists: Vec<ExportPlaylist>,
    #[serde(default)]
    pub rules: Vec<ExportRule>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ImportReport {
    pub tags_added: usize,
    pub tags_skipped: usize,
    pub playlists_added: usize,
    pub playlists_skipped: usize,
    pub rules_added: usize,
    pub rules_skipped: usize,
    /// Names of rules left out because their pattern (or name) is not valid.
    pub rules_invalid: Vec<String>,
}

impl ExportFile {
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| Error::Msg(format!("cannot serialize the export: {e}")))
    }

    /// Parse and check an export file.
    pub fn parse(json: &str) -> Result<Self> {
        let file: ExportFile =
            serde_json::from_str(json).map_err(|e| Error::Msg(format!("not a valid Kadence Media export file: {e}")))?;
        if file.format != FORMAT {
            return Err(Error::Msg("not a Kadence Media export file".into()));
        }
        if file.version > VERSION {
            return Err(Error::Msg(format!(
                "this file was made by a newer version of the app (format {}, supported {VERSION})",
                file.version
            )));
        }
        Ok(file)
    }
}

/// Id of the row named `name` (case-insensitive: the columns are NOCASE), creating it if missing.
/// The bool says whether it was created.
fn tag_id(tx: &Transaction, name: &str, color: &str) -> Result<(i64, bool)> {
    if let Some(id) = tx.query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| r.get(0)).optional()? {
        return Ok((id, false));
    }
    tx.execute("INSERT INTO tags(name, color) VALUES (?1, ?2)", params![name, color])?;
    Ok((tx.last_insert_rowid(), true))
}

fn playlist_id(tx: &Transaction, name: &str) -> Result<(i64, bool)> {
    if let Some(id) = tx.query_row("SELECT id FROM playlists WHERE name = ?1", params![name], |r| r.get(0)).optional()? {
        return Ok((id, false));
    }
    tx.execute("INSERT INTO playlists(name) VALUES (?1)", params![name])?;
    Ok((tx.last_insert_rowid(), true))
}

impl Library {
    /// Collect the requested sections (each can be left out).
    pub fn export_data(&self, tags: bool, playlists: bool, rules: bool) -> Result<ExportFile> {
        let mut file = ExportFile { format: FORMAT.into(), version: VERSION, tags: vec![], playlists: vec![], rules: vec![] };
        if tags {
            file.tags = self.list_tags()?.into_iter().map(|t| ExportTag { name: t.name, color: t.color }).collect();
        }
        if playlists {
            file.playlists = self.list_playlists()?.into_iter().map(|p| ExportPlaylist { name: p.name }).collect();
        }
        if rules {
            let names = |sql: &str, id: i64| -> Result<Vec<String>> {
                let mut st = self.conn.prepare(sql)?;
                let rows = st.query_map(params![id], |r| r.get(0))?;
                Ok(rows.collect::<std::result::Result<_, _>>()?)
            };
            for r in self.list_rules()? {
                file.rules.push(ExportRule {
                    tags: names(
                        "SELECT t.name FROM rule_tags rt JOIN tags t ON t.id = rt.tag_id WHERE rt.rule_id = ?1 ORDER BY t.name COLLATE NOCASE",
                        r.id,
                    )?,
                    playlists: names(
                        "SELECT p.name FROM rule_playlists rp JOIN playlists p ON p.id = rp.playlist_id WHERE rp.rule_id = ?1 ORDER BY p.name COLLATE NOCASE",
                        r.id,
                    )?,
                    name: r.name,
                    pattern: r.pattern,
                    case_insensitive: r.case_insensitive,
                    enabled: r.enabled,
                    hide: r.hide,
                });
            }
        }
        Ok(file)
    }

    /// Merge a file into this library, all or nothing. Rules are stored but not applied to
    /// existing videos (the Rules page does that on demand).
    pub fn import_data(&mut self, file: &ExportFile) -> Result<ImportReport> {
        let tx = self.conn.transaction()?;
        let mut report = ImportReport::default();

        for t in &file.tags {
            let name = t.name.trim();
            if name.is_empty() {
                report.tags_skipped += 1;
                continue;
            }
            let color = if t.color.trim().is_empty() { DEFAULT_TAG_COLOR } else { t.color.trim() };
            if tag_id(&tx, name, color)?.1 {
                report.tags_added += 1;
            } else {
                report.tags_skipped += 1;
            }
        }
        for p in &file.playlists {
            let name = p.name.trim();
            if name.is_empty() {
                report.playlists_skipped += 1;
                continue;
            }
            if playlist_id(&tx, name)?.1 {
                report.playlists_added += 1;
            } else {
                report.playlists_skipped += 1;
            }
        }
        for r in &file.rules {
            let name = r.name.trim();
            if name.is_empty() || crate::rules::compile(&r.pattern, r.case_insensitive).is_err() {
                report.rules_invalid.push(if name.is_empty() { r.pattern.clone() } else { name.to_string() });
                continue;
            }
            // A rule that is already here (same name, pattern and options) is not added again.
            let exists: bool = tx
                .prepare(
                    "SELECT 1 FROM rules WHERE name = ?1 COLLATE NOCASE AND pattern = ?2 AND case_insensitive = ?3 AND hide = ?4",
                )?
                .exists(params![name, r.pattern, r.case_insensitive as i64, r.hide as i64])?;
            if exists {
                report.rules_skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT INTO rules(name, pattern, case_insensitive, enabled, hide) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, r.pattern, r.case_insensitive as i64, r.enabled as i64, r.hide as i64],
            )?;
            let rule = tx.last_insert_rowid();
            for t in r.tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
                let (id, created) = tag_id(&tx, t, DEFAULT_TAG_COLOR)?;
                report.tags_added += created as usize;
                tx.execute("INSERT OR IGNORE INTO rule_tags(rule_id, tag_id) VALUES (?1, ?2)", params![rule, id])?;
            }
            for p in r.playlists.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
                let (id, created) = playlist_id(&tx, p)?;
                report.playlists_added += created as usize;
                tx.execute("INSERT OR IGNORE INTO rule_playlists(rule_id, playlist_id) VALUES (?1, ?2)", params![rule, id])?;
            }
            report.rules_added += 1;
        }
        tx.commit()?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RuleInput;

    fn source() -> Library {
        let mut lib = Library::open_in_memory().unwrap();
        // Burn some ids so they differ from the target's.
        lib.create_tag("filler", "#000000").unwrap();
        let tag = lib.create_tag("Action", "#ff0000").unwrap();
        lib.create_tag("Drama", "#00ff00").unwrap();
        let pl = lib.create_playlist("Weekend").unwrap();
        lib.create_playlist("Empty").unwrap();
        lib.conn.execute("INSERT INTO folders(id, path) VALUES (1, '/m')", []).unwrap();
        lib.conn
            .execute("INSERT INTO videos(id, path, folder_id, title) VALUES (1, '/m/a.mp4', 1, 'a')", [])
            .unwrap();
        lib.set_video_tag(1, tag.id, true).unwrap();
        lib.add_to_playlist(pl.id, 1).unwrap();
        lib.create_rule(&RuleInput {
            name: "Action movies".into(),
            pattern: "action".into(),
            case_insensitive: true,
            enabled: false,
            hide: true,
            tag_ids: vec![tag.id],
            playlist_ids: vec![pl.id],
        })
        .unwrap();
        lib
    }

    #[test]
    fn round_trip_between_libraries_with_different_ids() {
        let src = source();
        let json = src.export_data(true, true, true).unwrap().to_json().unwrap();
        // Definitions only: no video information leaks into the file.
        assert!(!json.contains("a.mp4") && !json.contains("/m"));

        let mut dst = Library::open_in_memory().unwrap();
        dst.create_tag("another", "#123456").unwrap();
        let report = dst.import_data(&ExportFile::parse(&json).unwrap()).unwrap();
        assert_eq!(
            report,
            ImportReport { tags_added: 3, tags_skipped: 0, playlists_added: 2, playlists_skipped: 0, rules_added: 1, rules_skipped: 0, rules_invalid: vec![] }
        );
        let tags = dst.list_tags().unwrap();
        assert_eq!(tags.iter().find(|t| t.name == "Action").unwrap().color, "#ff0000");
        assert!(tags.iter().all(|t| t.video_count == 0));
        assert!(dst.list_playlists().unwrap().iter().all(|p| p.video_count == 0));

        let rule = &dst.list_rules().unwrap()[0];
        assert_eq!((rule.name.as_str(), rule.pattern.as_str(), rule.enabled, rule.hide), ("Action movies", "action", false, true));
        let action = tags.iter().find(|t| t.name == "Action").unwrap().id;
        let weekend = dst.list_playlists().unwrap().iter().find(|p| p.name == "Weekend").unwrap().id;
        assert_eq!((rule.tag_ids.clone(), rule.playlist_ids.clone()), (vec![action], vec![weekend]));
    }

    #[test]
    fn importing_twice_changes_nothing() {
        let file = source().export_data(true, true, true).unwrap();
        let mut dst = Library::open_in_memory().unwrap();
        dst.import_data(&file).unwrap();
        let again = dst.import_data(&file).unwrap();
        assert_eq!(
            again,
            ImportReport { tags_added: 0, tags_skipped: 3, playlists_added: 0, playlists_skipped: 2, rules_added: 0, rules_skipped: 1, rules_invalid: vec![] }
        );
        assert_eq!((dst.list_tags().unwrap().len(), dst.list_playlists().unwrap().len(), dst.list_rules().unwrap().len()), (3, 2, 1));
    }

    #[test]
    fn existing_tags_keep_their_color_and_names_match_case_insensitively() {
        let file = source().export_data(true, false, false).unwrap();
        let mut dst = Library::open_in_memory().unwrap();
        dst.create_tag("ACTION", "#abcdef").unwrap();
        let r = dst.import_data(&file).unwrap();
        assert_eq!((r.tags_added, r.tags_skipped), (2, 1));
        assert_eq!(dst.list_tags().unwrap().iter().find(|t| t.name == "ACTION").unwrap().color, "#abcdef");
    }

    #[test]
    fn sections_can_be_left_out_and_rules_create_missing_links() {
        let src = source();
        let f = src.export_data(false, false, true).unwrap();
        assert!(f.tags.is_empty() && f.playlists.is_empty() && f.rules.len() == 1);
        let mut dst = Library::open_in_memory().unwrap();
        let r = dst.import_data(&f).unwrap();
        assert_eq!((r.tags_added, r.playlists_added, r.rules_added), (1, 1, 1));
        assert_eq!(dst.list_tags().unwrap()[0].color, DEFAULT_TAG_COLOR);
    }

    #[test]
    fn invalid_rules_are_skipped_and_reported() {
        let mut file = source().export_data(false, false, true).unwrap();
        file.rules[0].pattern = "(".into();
        file.rules.push(ExportRule { name: "".into(), pattern: "x".into(), case_insensitive: true, enabled: true, hide: false, tags: vec![], playlists: vec![] });
        let mut dst = Library::open_in_memory().unwrap();
        let r = dst.import_data(&file).unwrap();
        assert_eq!(r.rules_added, 0);
        assert_eq!(r.rules_invalid, vec!["Action movies".to_string(), "x".to_string()]);
        assert!(dst.list_tags().unwrap().is_empty()); // nothing half-imported
    }

    #[test]
    fn rejects_foreign_or_newer_files() {
        assert!(ExportFile::parse("{}").is_err());
        assert!(ExportFile::parse(r#"{"format":"other","version":1}"#).is_err());
        assert!(ExportFile::parse(r#"{"format":"kadence-media-export","version":99}"#).is_err());
        assert!(ExportFile::parse(r#"{"format":"kadence-media-export","version":1}"#).is_ok());
    }
}
