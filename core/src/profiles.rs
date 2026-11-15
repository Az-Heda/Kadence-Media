//! Profile registry: a small SQLite database (`profiles.sqlite3`) listing the profiles and
//! their optional PINs. Every profile owns a separate library database file, so tags,
//! playlists, rules, folders and settings are fully isolated. The first profile ("Default")
//! uses the original library file, so databases created before profiles existed keep working.
//!
//! The PIN is only a gate in the UI, not encryption: the library files stay readable on disk.

use crate::error::{Error, Result};
use argon2::Argon2;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const REGISTRY_FILE: &str = "profiles.sqlite3";
pub const DEFAULT_PROFILE_ID: i64 = 1;
const DEFAULT_PROFILE_NAME: &str = "Default";
const MAX_NAME_LEN: usize = 40;
const PIN_LEN: std::ops::RangeInclusive<usize> = 4..=8;
/// Wrong PINs tolerated before the profile is locked for a while.
const FREE_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProfileInfo {
    pub id: i64,
    pub name: String,
    pub has_pin: bool,
    pub is_default: bool,
}

pub struct Profiles {
    conn: Connection,
    dir: PathBuf,
    /// Failed PIN attempts per profile: (count, locked until).
    attempts: HashMap<i64, (u32, Option<Instant>)>,
}

impl Profiles {
    /// Open (creating if needed) the registry in `dir`. `default_file` is the library file of the
    /// default profile, used only the first time (an existing registry keeps its own value).
    pub fn open(dir: &Path, default_file: &str) -> Result<Profiles> {
        let conn = Connection::open(dir.join(REGISTRY_FILE))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profiles (
                id       INTEGER PRIMARY KEY,
                name     TEXT NOT NULL UNIQUE COLLATE NOCASE,
                db_file  TEXT NOT NULL UNIQUE,
                pin_salt TEXT,
                pin_hash TEXT
            );",
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO profiles (id, name, db_file) VALUES (?1, ?2, ?3)",
            params![DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME, default_file],
        )?;
        Ok(Profiles { conn, dir: dir.to_path_buf(), attempts: HashMap::new() })
    }

    pub fn list(&self) -> Result<Vec<ProfileInfo>> {
        let mut stmt = self.conn.prepare("SELECT id, name, pin_hash IS NOT NULL FROM profiles ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            let id: i64 = r.get(0)?;
            Ok(ProfileInfo { id, name: r.get(1)?, has_pin: r.get(2)?, is_default: id == DEFAULT_PROFILE_ID })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn info(&self, id: i64) -> Result<ProfileInfo> {
        self.list()?.into_iter().find(|p| p.id == id).ok_or_else(|| Error::Msg("profile not found".into()))
    }

    /// Path of the library database of profile `id`.
    pub fn db_path(&self, id: i64) -> Result<PathBuf> {
        let file: Option<String> = self
            .conn
            .query_row("SELECT db_file FROM profiles WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        file.map(|f| self.dir.join(f)).ok_or_else(|| Error::Msg("profile not found".into()))
    }

    pub fn create(&mut self, name: &str, pin: Option<&str>) -> Result<ProfileInfo> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Msg("the profile name cannot be empty".into()));
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(Error::Msg(format!("the profile name is too long (max {MAX_NAME_LEN} characters)")));
        }
        let secret = pin.map(hash_pin).transpose()?;
        let tx = self.conn.transaction()?;
        // The file name needs the id, which only exists after the insert.
        let inserted = tx.execute(
            "INSERT INTO profiles (name, db_file, pin_salt, pin_hash) VALUES (?1, '', ?2, ?3)",
            params![name, secret.as_ref().map(|s| &s.0), secret.as_ref().map(|s| &s.1)],
        );
        if let Err(e) = inserted {
            return Err(if is_unique_violation(&e) { Error::Msg("a profile with this name already exists".into()) } else { e.into() });
        }
        let id = tx.last_insert_rowid();
        tx.execute("UPDATE profiles SET db_file = ?1 WHERE id = ?2", params![format!("profile-{id}.sqlite3"), id])?;
        tx.commit()?;
        self.info(id)
    }

    /// Check the PIN of profile `id` (anything is accepted if it has none). Wrong guesses are
    /// rate limited: after a few of them the profile is locked for a growing delay.
    pub fn verify(&mut self, id: i64, pin: Option<&str>) -> Result<()> {
        if let Some((_, Some(until))) = self.attempts.get(&id) {
            let now = Instant::now();
            if *until > now {
                let secs = (*until - now).as_secs() + 1;
                return Err(Error::Msg(format!("too many wrong PINs, try again in {secs} s")));
            }
        }
        let stored: Option<(Option<String>, Option<String>)> = self
            .conn
            .query_row("SELECT pin_salt, pin_hash FROM profiles WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((salt, hash)) = stored else { return Err(Error::Msg("profile not found".into())) };
        let (Some(salt), Some(hash)) = (salt, hash) else { return Ok(()) };
        let ok = pin.map(|p| hash_with_salt(p, &salt).map(|h| ct_eq(&h, &hash))).transpose()?.unwrap_or(false);
        if ok {
            self.attempts.remove(&id);
            return Ok(());
        }
        let entry = self.attempts.entry(id).or_insert((0, None));
        entry.0 += 1;
        if entry.0 >= FREE_ATTEMPTS {
            // 5 s, 10 s, 20 s ... capped at 5 minutes.
            let secs = (5u64 << (entry.0 - FREE_ATTEMPTS).min(6)).min(300);
            entry.1 = Some(Instant::now() + Duration::from_secs(secs));
        }
        Err(Error::Msg("wrong PIN".into()))
    }

    /// Set, change or remove (`new_pin = None`) the PIN. Needs the current PIN if there is one.
    pub fn set_pin(&mut self, id: i64, current_pin: Option<&str>, new_pin: Option<&str>) -> Result<()> {
        self.verify(id, current_pin)?;
        let secret = new_pin.map(hash_pin).transpose()?;
        self.conn.execute(
            "UPDATE profiles SET pin_salt = ?1, pin_hash = ?2 WHERE id = ?3",
            params![secret.as_ref().map(|s| &s.0), secret.as_ref().map(|s| &s.1), id],
        )?;
        Ok(())
    }

    /// Delete a profile and its library file. The default profile cannot be deleted.
    pub fn delete(&mut self, id: i64, pin: Option<&str>) -> Result<()> {
        if id == DEFAULT_PROFILE_ID {
            return Err(Error::Msg("the default profile cannot be deleted".into()));
        }
        self.verify(id, pin)?;
        let path = self.db_path(id)?;
        self.conn.execute("DELETE FROM profiles WHERE id = ?1", [id])?;
        self.attempts.remove(&id);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(path.with_file_name(format!("{name}{suffix}")));
        }
        Ok(())
    }
}

fn is_unique_violation(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation)
}

/// PINs are 4 to 8 digits.
fn validate_pin(pin: &str) -> Result<()> {
    if PIN_LEN.contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err(Error::Msg(format!("the PIN must be {} to {} digits", PIN_LEN.start(), PIN_LEN.end())))
    }
}

fn hash_with_salt(pin: &str, salt_hex: &str) -> Result<String> {
    let salt = from_hex(salt_hex).ok_or_else(|| Error::Msg("corrupted PIN data".into()))?;
    let mut out = [0u8; 32];
    Argon2::default().hash_password_into(pin.as_bytes(), &salt, &mut out).map_err(|e| Error::Msg(e.to_string()))?;
    Ok(to_hex(&out))
}

/// (salt, hash) as hex strings.
fn hash_pin(pin: &str) -> Result<(String, String)> {
    validate_pin(pin)?;
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|e| Error::Msg(e.to_string()))?;
    let salt = to_hex(&salt);
    let hash = hash_with_salt(pin, &salt)?;
    Ok((salt, hash))
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// Compare without stopping at the first difference.
fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> (tempfile::TempDir, Profiles) {
        let dir = tempfile::tempdir().unwrap();
        let p = Profiles::open(dir.path(), "KadenceMedia.sqlite3").unwrap();
        (dir, p)
    }

    #[test]
    fn default_profile_uses_the_original_library_file() {
        let (dir, p) = open();
        assert_eq!(p.list().unwrap(), vec![ProfileInfo { id: 1, name: "Default".into(), has_pin: false, is_default: true }]);
        assert_eq!(p.db_path(1).unwrap(), dir.path().join("KadenceMedia.sqlite3"));
        // Reopening keeps the registry as it was.
        let p = Profiles::open(dir.path(), "other.sqlite3").unwrap();
        assert_eq!(p.db_path(1).unwrap(), dir.path().join("KadenceMedia.sqlite3"));
        assert_eq!(p.list().unwrap().len(), 1);
    }

    #[test]
    fn create_validates_names_and_pins() {
        let (_d, mut p) = open();
        assert!(p.create("  ", None).is_err());
        assert!(p.create(&"x".repeat(41), None).is_err());
        assert!(p.create("Kids", Some("12")).is_err());
        assert!(p.create("Kids", Some("12ab")).is_err());
        let kids = p.create(" Kids ", Some("1234")).unwrap();
        assert_eq!((kids.name.as_str(), kids.has_pin, kids.is_default), ("Kids", true, false));
        assert!(p.create("kids", None).unwrap_err().to_string().contains("already exists"));
        assert!(p.create("default", None).is_err());
        let plain = p.create("Guests", None).unwrap();
        assert!(!plain.has_pin);
        assert_ne!(p.db_path(kids.id).unwrap(), p.db_path(plain.id).unwrap());
        assert_eq!(p.list().unwrap().len(), 3);
    }

    #[test]
    fn pin_is_checked_and_stored_hashed() {
        let (dir, mut p) = open();
        let id = p.create("Kids", Some("4321")).unwrap().id;
        assert!(p.verify(id, Some("4321")).is_ok());
        assert!(p.verify(id, Some("1234")).is_err());
        assert!(p.verify(id, None).is_err());
        assert!(p.verify(1, None).is_ok()); // no PIN: anything goes
        let raw = Connection::open(dir.path().join(REGISTRY_FILE)).unwrap();
        let stored: String = raw.query_row("SELECT pin_hash FROM profiles WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert!(!stored.contains("4321"));
    }

    #[test]
    fn repeated_wrong_pins_lock_the_profile() {
        let (_d, mut p) = open();
        let id = p.create("Kids", Some("4321")).unwrap().id;
        for _ in 0..FREE_ATTEMPTS {
            assert_eq!(p.verify(id, Some("0000")).unwrap_err().to_string(), "wrong PIN");
        }
        // Even the right PIN is refused while locked.
        assert!(p.verify(id, Some("4321")).unwrap_err().to_string().contains("try again"));
    }

    #[test]
    fn pin_can_be_changed_and_removed() {
        let (_d, mut p) = open();
        let id = p.create("Kids", Some("4321")).unwrap().id;
        assert!(p.set_pin(id, Some("9999"), Some("1111")).is_err()); // wrong current PIN
        p.set_pin(id, Some("4321"), Some("1111")).unwrap();
        assert!(p.verify(id, Some("1111")).is_ok());
        p.set_pin(id, Some("1111"), None).unwrap();
        assert!(!p.list().unwrap()[1].has_pin);
        p.set_pin(id, None, Some("2222")).unwrap(); // adding a PIN to an open profile
        assert!(p.list().unwrap()[1].has_pin);
    }

    #[test]
    fn delete_removes_the_profile_and_its_file() {
        let (_d, mut p) = open();
        assert!(p.delete(1, None).is_err());
        let id = p.create("Kids", Some("4321")).unwrap().id;
        let path = p.db_path(id).unwrap();
        std::fs::write(&path, b"x").unwrap();
        assert!(p.delete(id, Some("0000")).is_err());
        assert!(path.exists());
        p.delete(id, Some("4321")).unwrap();
        assert!(!path.exists());
        assert_eq!(p.list().unwrap().len(), 1);
        assert!(p.db_path(id).is_err());
    }
}
