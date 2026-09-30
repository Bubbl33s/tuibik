//! `store` — SQLite persistence for sessions, solves, and app config.
//!
//! Schema (created idempotently on open):
//! - `sessions(id, name, created_at)`
//! - `solves(id, session_id, time_ms, scramble, created_at, penalty)`
//! - `config(key, value)`  — simple key/value for active session, theme, etc.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

/// A WCA-style penalty attached to a solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Penalty {
    #[default]
    Ok,
    PlusTwo,
    Dnf,
}

impl Penalty {
    /// Integer encoding persisted in the database (0 = OK, 1 = +2, 2 = DNF).
    pub fn to_i64(self) -> i64 {
        match self {
            Penalty::Ok => 0,
            Penalty::PlusTwo => 1,
            Penalty::Dnf => 2,
        }
    }

    /// Decode a stored value; unknown values map to `Ok`.
    pub fn from_i64(v: i64) -> Self {
        match v {
            1 => Penalty::PlusTwo,
            2 => Penalty::Dnf,
            _ => Penalty::Ok,
        }
    }
}

/// A single recorded solve. This is the shared model consumed by the `stats`
/// crate as well.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solve {
    pub id: i64,
    pub session_id: i64,
    /// Solve time in milliseconds.
    pub time_ms: i64,
    /// The scramble that was solved (WCA notation).
    pub scramble: String,
    /// Unix epoch seconds when the solve was recorded.
    pub created_at: i64,
    /// Penalty applied to this solve.
    pub penalty: Penalty,
}

/// A named session grouping solves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}

/// The persistence handle.
pub struct Store {
    conn: Connection,
}

/// Errors from the store layer.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not determine data directory")]
    NoDataDir,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, StoreError>;

/// Default location for the tuibik database in the user's data directory.
pub fn default_db_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "tuibik").ok_or(StoreError::NoDataDir)?;
    let dir = dirs.data_dir().to_path_buf();
    Ok(dir.join("tuibik.sqlite"))
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Store {
    /// Open (creating if necessary) the database at `path`, running migrations.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Open the database at the default data-directory path.
    pub fn open_default() -> Result<Self> {
        let path = default_db_path()?;
        Self::open(path)
    }

    /// Open an in-memory database (for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Idempotent schema creation.
    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS sessions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                created_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS solves (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id  INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                time_ms     INTEGER NOT NULL,
                scramble    TEXT NOT NULL,
                created_at  INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_solves_session
                ON solves(session_id, id);

            CREATE TABLE IF NOT EXISTS config (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        self.migrate_penalty()?;
        Ok(())
    }

    /// Add the `penalty` column to databases created before penalties existed,
    /// inside a transaction, and bump `user_version` to 1.
    fn migrate_penalty(&self) -> Result<()> {
        let has_penalty: bool = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(solves)")?;
            let names = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            names.iter().any(|n| n == "penalty")
        };
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if has_penalty && version >= 1 {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        if !has_penalty {
            tx.execute_batch("ALTER TABLE solves ADD COLUMN penalty INTEGER NOT NULL DEFAULT 0;")?;
        }
        tx.execute_batch("PRAGMA user_version = 1;")?;
        tx.commit()?;
        Ok(())
    }

    // ===== Sessions =====

    /// Create a new session, returning its id.
    pub fn create_session(&self, name: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (name, created_at) VALUES (?1, ?2)",
            params![name, now_epoch()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Rename an existing session.
    pub fn rename_session(&self, id: i64, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(())
    }

    /// Delete a session and all its solves (via ON DELETE CASCADE).
    pub fn delete_session(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// List all sessions, oldest first.
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM sessions ORDER BY id ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Session {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Fetch a session by id.
    pub fn get_session(&self, id: i64) -> Result<Option<Session>> {
        let s = self
            .conn
            .query_row(
                "SELECT id, name, created_at FROM sessions WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Session {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        created_at: row.get(2)?,
                    })
                },
            )
            .optional()?;
        Ok(s)
    }

    /// Ensure at least one session exists; return the active session id,
    /// creating a default "Session 1" if the database is empty.
    pub fn ensure_default_session(&self) -> Result<i64> {
        let sessions = self.list_sessions()?;
        if let Some(first) = sessions.first() {
            Ok(first.id)
        } else {
            self.create_session("Session 1")
        }
    }

    // ===== Solves =====

    /// Insert a solve, returning its id.
    pub fn add_solve(
        &self,
        session_id: i64,
        time_ms: i64,
        scramble: &str,
        penalty: Penalty,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO solves (session_id, time_ms, scramble, created_at, penalty)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, time_ms, scramble, now_epoch(), penalty.to_i64()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Set the penalty of an existing solve.
    pub fn set_penalty(&self, id: i64, penalty: Penalty) -> Result<()> {
        self.conn.execute(
            "UPDATE solves SET penalty = ?1 WHERE id = ?2",
            params![penalty.to_i64(), id],
        )?;
        Ok(())
    }

    /// Delete a single solve by id.
    pub fn delete_solve(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM solves WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// List solves for a session, oldest first (chronological order).
    pub fn list_solves(&self, session_id: i64) -> Result<Vec<Solve>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, time_ms, scramble, created_at, penalty
             FROM solves WHERE session_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(Solve {
                id: row.get(0)?,
                session_id: row.get(1)?,
                time_ms: row.get(2)?,
                scramble: row.get(3)?,
                created_at: row.get(4)?,
                penalty: Penalty::from_i64(row.get(5)?),
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    // ===== Config (key/value) =====

    /// Set a config value.
    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get a config value.
    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let v = self
            .conn
            .query_row(
                "SELECT value FROM config WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(v)
    }

    /// Convenience: get the active session id from config, if any.
    pub fn active_session_id(&self) -> Result<Option<i64>> {
        Ok(self
            .get_config("active_session")?
            .and_then(|s| s.parse().ok()))
    }

    /// Convenience: persist the active session id.
    pub fn set_active_session(&self, id: i64) -> Result<()> {
        self.set_config("active_session", &id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_is_idempotent() {
        let store = Store::open_in_memory().unwrap();
        // Running migrate again must not error.
        store.migrate().unwrap();
        store.migrate().unwrap();
    }

    #[test]
    fn create_and_list_sessions() {
        let store = Store::open_in_memory().unwrap();
        let a = store.create_session("Practice").unwrap();
        let b = store.create_session("OH").unwrap();
        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, a);
        assert_eq!(sessions[0].name, "Practice");
        assert_eq!(sessions[1].id, b);
        assert_eq!(sessions[1].name, "OH");
    }

    #[test]
    fn rename_session_works() {
        let store = Store::open_in_memory().unwrap();
        let id = store.create_session("Old").unwrap();
        store.rename_session(id, "New").unwrap();
        assert_eq!(store.get_session(id).unwrap().unwrap().name, "New");
    }

    #[test]
    fn insert_and_list_solves() {
        let store = Store::open_in_memory().unwrap();
        let sid = store.create_session("S").unwrap();
        store
            .add_solve(sid, 12345, "R U R' U'", Penalty::Ok)
            .unwrap();
        store.add_solve(sid, 9876, "F2 B2", Penalty::Ok).unwrap();
        store.add_solve(sid, 11111, "L D", Penalty::Ok).unwrap();
        let solves = store.list_solves(sid).unwrap();
        assert_eq!(solves.len(), 3);
        assert_eq!(solves[0].time_ms, 12345);
        assert_eq!(solves[0].scramble, "R U R' U'");
        assert_eq!(solves[1].time_ms, 9876);
        assert_eq!(solves[2].time_ms, 11111);
    }

    #[test]
    fn solves_are_filtered_by_session() {
        let store = Store::open_in_memory().unwrap();
        let s1 = store.create_session("One").unwrap();
        let s2 = store.create_session("Two").unwrap();
        store.add_solve(s1, 100, "A", Penalty::Ok).unwrap();
        store.add_solve(s1, 200, "B", Penalty::Ok).unwrap();
        store.add_solve(s2, 300, "C", Penalty::Ok).unwrap();
        assert_eq!(store.list_solves(s1).unwrap().len(), 2);
        assert_eq!(store.list_solves(s2).unwrap().len(), 1);
        assert_eq!(store.list_solves(s2).unwrap()[0].time_ms, 300);
    }

    #[test]
    fn deleting_session_cascades_solves() {
        let store = Store::open_in_memory().unwrap();
        let sid = store.create_session("Temp").unwrap();
        store.add_solve(sid, 500, "X", Penalty::Ok).unwrap();
        store.add_solve(sid, 600, "Y", Penalty::Ok).unwrap();
        store.delete_session(sid).unwrap();
        assert!(store.get_session(sid).unwrap().is_none());
        assert_eq!(store.list_solves(sid).unwrap().len(), 0);
    }

    #[test]
    fn delete_single_solve() {
        let store = Store::open_in_memory().unwrap();
        let sid = store.create_session("S").unwrap();
        let id = store.add_solve(sid, 500, "X", Penalty::Ok).unwrap();
        store.add_solve(sid, 600, "Y", Penalty::Ok).unwrap();
        store.delete_solve(id).unwrap();
        let solves = store.list_solves(sid).unwrap();
        assert_eq!(solves.len(), 1);
        assert_eq!(solves[0].time_ms, 600);
    }

    #[test]
    fn config_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.get_config("theme").unwrap(), None);
        store.set_config("theme", "nord").unwrap();
        assert_eq!(store.get_config("theme").unwrap().as_deref(), Some("nord"));
        // Upsert overwrites.
        store.set_config("theme", "dracula").unwrap();
        assert_eq!(
            store.get_config("theme").unwrap().as_deref(),
            Some("dracula")
        );
    }

    #[test]
    fn active_session_helpers() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.active_session_id().unwrap(), None);
        let sid = store.create_session("S").unwrap();
        store.set_active_session(sid).unwrap();
        assert_eq!(store.active_session_id().unwrap(), Some(sid));
    }

    #[test]
    fn ensure_default_session_creates_when_empty() {
        let store = Store::open_in_memory().unwrap();
        let id = store.ensure_default_session().unwrap();
        assert!(id > 0);
        // Second call returns the same (first) session, does not create another.
        let again = store.ensure_default_session().unwrap();
        assert_eq!(id, again);
        assert_eq!(store.list_sessions().unwrap().len(), 1);
    }

    #[test]
    fn migration_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("tuibik-test-{}", std::process::id()));
        let path = dir.join("t.sqlite");
        let _ = std::fs::remove_dir_all(&dir);
        {
            let store = Store::open(&path).unwrap();
            let sid = store.create_session("Persisted").unwrap();
            store.add_solve(sid, 4242, "R", Penalty::Ok).unwrap();
        }
        {
            let store = Store::open(&path).unwrap();
            let sessions = store.list_sessions().unwrap();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].name, "Persisted");
            assert_eq!(store.list_solves(sessions[0].id).unwrap()[0].time_ms, 4242);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn penalty_roundtrip_and_unknown_is_ok() {
        for p in [Penalty::Ok, Penalty::PlusTwo, Penalty::Dnf] {
            assert_eq!(Penalty::from_i64(p.to_i64()), p);
        }
        assert_eq!(Penalty::from_i64(99), Penalty::Ok);
        assert_eq!(Penalty::from_i64(-1), Penalty::Ok);
    }

    #[test]
    fn migrates_old_schema_keeping_solves() {
        let dir = std::env::temp_dir().join(format!("tuibik-old-{}", std::process::id()));
        let path = dir.join("old.sqlite");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE sessions (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, created_at INTEGER NOT NULL);
                CREATE TABLE solves (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                    time_ms INTEGER NOT NULL, scramble TEXT NOT NULL, created_at INTEGER NOT NULL);
                CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                INSERT INTO sessions (name, created_at) VALUES ('Old', 0);
                "#,
            )
            .unwrap();
            for i in 0..50 {
                conn.execute(
                    "INSERT INTO solves (session_id, time_ms, scramble, created_at) VALUES (1, ?1, 'R', 0)",
                    params![10_000 + i],
                )
                .unwrap();
            }
        }
        let store = Store::open(&path).unwrap();
        let solves = store.list_solves(1).unwrap();
        assert_eq!(solves.len(), 50);
        assert!(solves.iter().all(|s| s.penalty == Penalty::Ok));
        assert_eq!(solves[7].time_ms, 10_007);
        let v: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 1);
        drop(store);
        // Reopening is a no-op.
        Store::open(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dnf_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("tuibik-dnf-{}", std::process::id()));
        let path = dir.join("t.sqlite");
        let _ = std::fs::remove_dir_all(&dir);
        let sid;
        {
            let store = Store::open(&path).unwrap();
            sid = store.create_session("S").unwrap();
            let id = store.add_solve(sid, 9000, "R", Penalty::Ok).unwrap();
            store.add_solve(sid, 8000, "U", Penalty::PlusTwo).unwrap();
            store.set_penalty(id, Penalty::Dnf).unwrap();
        }
        let store = Store::open(&path).unwrap();
        let solves = store.list_solves(sid).unwrap();
        assert_eq!(solves[0].penalty, Penalty::Dnf);
        assert_eq!(solves[0].time_ms, 9000);
        assert_eq!(solves[1].penalty, Penalty::PlusTwo);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
