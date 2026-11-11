//! Automatic rules: regex on a video's full path -> tags + playlists.

use crate::error::{Error, Result};
use crate::models::{Rule, RuleInput};
use crate::Library;
use regex::{Regex, RegexBuilder};
use rusqlite::{params, Connection};

pub(crate) struct CompiledRule {
    re: Regex,
    tag_ids: Vec<i64>,
    playlist_ids: Vec<i64>,
}

fn compile(pattern: &str, case_insensitive: bool) -> Result<Regex> {
    if pattern.is_empty() {
        return Err(Error::Msg("the pattern cannot be empty".into()));
    }
    RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
        .map_err(|e| Error::Msg(format!("invalid regular expression: {e}")))
}

fn ids(conn: &Connection, sql: &str, rule_id: i64) -> Result<Vec<i64>> {
    let mut st = conn.prepare(sql)?;
    let rows = st.query_map(params![rule_id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn tag_ids(conn: &Connection, rule_id: i64) -> Result<Vec<i64>> {
    ids(conn, "SELECT tag_id FROM rule_tags WHERE rule_id = ?1 ORDER BY tag_id", rule_id)
}

fn playlist_ids(conn: &Connection, rule_id: i64) -> Result<Vec<i64>> {
    ids(conn, "SELECT playlist_id FROM rule_playlists WHERE rule_id = ?1 ORDER BY playlist_id", rule_id)
}

/// Compile every enabled rule once (per scan).
pub(crate) fn load_enabled(conn: &Connection) -> Result<Vec<CompiledRule>> {
    let mut st = conn.prepare("SELECT id, pattern, case_insensitive FROM rules WHERE enabled = 1 ORDER BY id")?;
    let rows: Vec<(i64, String, bool)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut out = Vec::new();
    for (id, pattern, ci) in rows {
        // Patterns are validated on save; skip any that somehow don't compile.
        if let Ok(re) = compile(&pattern, ci) {
            out.push(CompiledRule { re, tag_ids: tag_ids(conn, id)?, playlist_ids: playlist_ids(conn, id)? });
        }
    }
    Ok(out)
}

/// Apply all matching rules to one video. Returns true if any rule matched.
pub(crate) fn apply(conn: &Connection, rules: &[CompiledRule], video_id: i64, path: &str) -> Result<bool> {
    let mut matched = false;
    for rule in rules.iter().filter(|r| r.re.is_match(path)) {
        matched = true;
        for t in &rule.tag_ids {
            conn.execute(
                "INSERT OR IGNORE INTO video_tags(video_id, tag_id) VALUES (?1, ?2)",
                params![video_id, t],
            )?;
        }
        for p in &rule.playlist_ids {
            conn.execute(
                "INSERT OR IGNORE INTO playlist_items(playlist_id, video_id, position)
                 VALUES (?1, ?2, COALESCE((SELECT MAX(position) + 1 FROM playlist_items WHERE playlist_id = ?1), 0))",
                params![p, video_id],
            )?;
        }
    }
    Ok(matched)
}

impl Library {
    pub fn list_rules(&self) -> Result<Vec<Rule>> {
        let mut st = self
            .conn
            .prepare("SELECT id, name, pattern, case_insensitive, enabled FROM rules ORDER BY id")?;
        let rows: Vec<(i64, String, String, bool, bool)> = st
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? != 0, r.get::<_, i64>(4)? != 0))
            })?
            .collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::new();
        for (id, name, pattern, case_insensitive, enabled) in rows {
            out.push(Rule {
                id,
                name,
                pattern,
                case_insensitive,
                enabled,
                tag_ids: tag_ids(&self.conn, id)?,
                playlist_ids: playlist_ids(&self.conn, id)?,
            });
        }
        Ok(out)
    }

    fn validate_rule(input: &RuleInput) -> Result<()> {
        if input.name.trim().is_empty() {
            return Err(Error::Msg("the rule name cannot be empty".into()));
        }
        compile(&input.pattern, input.case_insensitive).map(|_| ())
    }

    fn set_rule_links(&mut self, id: i64, input: &RuleInput) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM rule_tags WHERE rule_id = ?1", params![id])?;
        tx.execute("DELETE FROM rule_playlists WHERE rule_id = ?1", params![id])?;
        for t in &input.tag_ids {
            tx.execute("INSERT OR IGNORE INTO rule_tags(rule_id, tag_id) VALUES (?1, ?2)", params![id, t])?;
        }
        for p in &input.playlist_ids {
            tx.execute(
                "INSERT OR IGNORE INTO rule_playlists(rule_id, playlist_id) VALUES (?1, ?2)",
                params![id, p],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn create_rule(&mut self, input: &RuleInput) -> Result<Rule> {
        Self::validate_rule(input)?;
        self.conn.execute(
            "INSERT INTO rules(name, pattern, case_insensitive, enabled) VALUES (?1, ?2, ?3, ?4)",
            params![input.name.trim(), input.pattern, input.case_insensitive as i64, input.enabled as i64],
        )?;
        let id = self.conn.last_insert_rowid();
        self.set_rule_links(id, input)?;
        self.list_rules()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| Error::Msg("rule vanished".into()))
    }

    pub fn update_rule(&mut self, id: i64, input: &RuleInput) -> Result<()> {
        Self::validate_rule(input)?;
        let n = self.conn.execute(
            "UPDATE rules SET name = ?1, pattern = ?2, case_insensitive = ?3, enabled = ?4 WHERE id = ?5",
            params![input.name.trim(), input.pattern, input.case_insensitive as i64, input.enabled as i64, id],
        )?;
        if n == 0 {
            return Err(Error::Msg(format!("rule {id} not found")));
        }
        self.set_rule_links(id, input)
    }

    pub fn delete_rule(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM rules WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Re-apply every enabled rule to every video already in the library
    /// (additive and idempotent). Returns how many videos matched a rule.
    pub fn apply_rules_to_existing(&mut self) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let rules = load_enabled(&tx)?;
        let videos: Vec<(i64, String)> = {
            let mut st = tx.prepare("SELECT id, path FROM videos ORDER BY path")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let mut matched = 0;
        for (id, path) in &videos {
            if apply(&tx, &rules, *id, path)? {
                matched += 1;
            }
        }
        tx.commit()?;
        Ok(matched)
    }

    /// How many library videos a pattern would match (for the rule editor preview).
    pub fn count_pattern_matches(&self, pattern: &str, case_insensitive: bool) -> Result<usize> {
        let re = compile(pattern, case_insensitive)?;
        let mut st = self.conn.prepare("SELECT path FROM videos")?;
        let mut n = 0;
        for row in st.query_map([], |r| r.get::<_, String>(0))? {
            if re.is_match(&row?) {
                n += 1;
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{RuleInput, VideoFilter};
    use crate::Library;
    use std::fs;
    use std::path::Path;

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    fn input(pattern: &str, tags: Vec<i64>, playlists: Vec<i64>) -> RuleInput {
        RuleInput {
            name: "r".into(),
            pattern: pattern.into(),
            case_insensitive: true,
            enabled: true,
            tag_ids: tags,
            playlist_ids: playlists,
        }
    }

    #[test]
    fn rule_applies_to_new_videos_only_and_not_on_rescan() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "Comedy/a.mp4");
        touch(dir.path(), "Drama/b.mp4");
        let mut lib = Library::open_in_memory().unwrap();
        let tag = lib.create_tag("funny", "#fff").unwrap();
        let pl = lib.create_playlist("Laughs").unwrap();
        lib.create_rule(&input("comedy", vec![tag.id], vec![pl.id])).unwrap();

        let f = lib.add_folder(dir.path().to_str().unwrap()).unwrap();
        let r = lib.scan_folder(f.id).unwrap();
        assert_eq!((r.added, r.rule_matches), (2, 1));
        let vids = lib.list_videos(&VideoFilter::default()).unwrap();
        let a = vids.iter().find(|v| v.title == "a").unwrap();
        let b = vids.iter().find(|v| v.title == "b").unwrap();
        assert_eq!(a.tags.len(), 1);
        assert_eq!(a.playlist_ids, vec![pl.id]);
        assert!(b.tags.is_empty() && b.playlist_ids.is_empty());

        // User removes the tag by hand; a rescan must not bring it back.
        lib.set_video_tag(a.id, tag.id, false).unwrap();
        let r = lib.scan_folder(f.id).unwrap();
        assert_eq!((r.added, r.rule_matches), (0, 0));
        assert!(lib.get_video(a.id).unwrap().tags.is_empty());

        // ...but an explicit "apply to existing" does.
        assert_eq!(lib.apply_rules_to_existing().unwrap(), 1);
        assert_eq!(lib.get_video(a.id).unwrap().tags.len(), 1);
        assert_eq!(lib.apply_rules_to_existing().unwrap(), 1); // idempotent
        assert_eq!(lib.get_video(a.id).unwrap().playlist_ids, vec![pl.id]);
    }

    #[test]
    fn rules_apply_in_path_order_to_playlists() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["c.mp4", "a.mp4", "b.mp4"] {
            touch(dir.path(), n);
        }
        let mut lib = Library::open_in_memory().unwrap();
        let pl = lib.create_playlist("All").unwrap();
        lib.create_rule(&input(r"\.mp4$", vec![], vec![pl.id])).unwrap();
        let f = lib.add_folder(dir.path().to_str().unwrap()).unwrap();
        lib.scan_folder(f.id).unwrap();
        let titles: Vec<_> = lib
            .list_videos(&VideoFilter { playlist_id: Some(pl.id), ..Default::default() })
            .unwrap()
            .into_iter()
            .map(|v| v.title)
            .collect();
        assert_eq!(titles, vec!["a", "b", "c"]);
    }

    #[test]
    fn invalid_or_empty_rules_are_rejected() {
        let mut lib = Library::open_in_memory().unwrap();
        assert!(lib.create_rule(&input("(unclosed", vec![], vec![])).is_err());
        assert!(lib.create_rule(&input("", vec![], vec![])).is_err());
        let mut bad_name = input("x", vec![], vec![]);
        bad_name.name = "  ".into();
        assert!(lib.create_rule(&bad_name).is_err());
        assert!(lib.count_pattern_matches("[", true).is_err());
        assert!(lib.list_rules().unwrap().is_empty());
    }

    #[test]
    fn disabled_rule_does_nothing_and_update_works() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "x/a.mp4");
        let mut lib = Library::open_in_memory().unwrap();
        let tag = lib.create_tag("t", "#fff").unwrap();
        let mut inp = input("x", vec![tag.id], vec![]);
        inp.enabled = false;
        let rule = lib.create_rule(&inp).unwrap();
        let f = lib.add_folder(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(lib.scan_folder(f.id).unwrap().rule_matches, 0);

        inp.enabled = true;
        inp.pattern = "X/A".into(); // case-insensitive by default
        lib.update_rule(rule.id, &inp).unwrap();
        assert_eq!(lib.list_rules().unwrap()[0].pattern, "X/A");
        assert_eq!(lib.apply_rules_to_existing().unwrap(), 1);
        assert_eq!(lib.count_pattern_matches("X/A", true).unwrap(), 1);
        assert_eq!(lib.count_pattern_matches("X/A", false).unwrap(), 0);
    }

    #[test]
    fn deleting_tag_or_playlist_cascades_out_of_rule() {
        let mut lib = Library::open_in_memory().unwrap();
        let tag = lib.create_tag("t", "#fff").unwrap();
        let pl = lib.create_playlist("p").unwrap();
        let rule = lib.create_rule(&input("x", vec![tag.id], vec![pl.id])).unwrap();
        lib.delete_tag(tag.id).unwrap();
        let r = &lib.list_rules().unwrap()[0];
        assert!(r.tag_ids.is_empty());
        assert_eq!(r.playlist_ids, vec![pl.id]);
        lib.delete_playlist(pl.id).unwrap();
        assert!(lib.list_rules().unwrap()[0].playlist_ids.is_empty());
        lib.delete_rule(rule.id).unwrap();
        assert!(lib.list_rules().unwrap().is_empty());
    }
}
