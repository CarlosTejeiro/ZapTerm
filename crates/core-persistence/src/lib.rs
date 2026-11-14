#![forbid(unsafe_code)]

//! `core-persistence` — SQLite storage and schema migrations.
//!
//! Credentials are never stored here; only opaque keyring references
//! live in the database (see `core-vault`).

pub mod error;

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

pub use error::Error;

// ---------------------------------------------------------------------------
// Public model types (serialisable for the Tauri bridge)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColorScheme {
    pub id: i64,
    pub name: String,
    pub palette_json: String,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionLog {
    pub id: i64,
    pub session_id: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub file_path: String,
    pub bytes: i64,
    pub format: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Folder {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub sort_order: i32,
}

/// A named, ordered set of saved sessions opened together for multi-host
/// orchestration. `session_ids` is the ordered membership (saved-session ids).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BroadcastGroup {
    pub id: i64,
    pub name: String,
    pub sort_order: i32,
    pub session_ids: Vec<i64>,
}

/// A port-forward saved on a session, auto-started when it connects.
/// `target_*` are `None` for dynamic (SOCKS5) forwards.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedForward {
    /// "local" (-L), "dynamic" (-D, SOCKS5) or "remote" (-R).
    pub kind: String,
    pub bind_addr: String,
    pub bind_port: u16,
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
    /// What this tunnel is for ("FortiGate GUI"…); `None` = unnamed. Absent
    /// in exports made before it existed.
    #[serde(default)]
    pub name: Option<String>,
}

/// A stored credential row. The secret itself never lives here — only the
/// opaque `keyring_key` that locates it in the OS vault.
#[derive(Debug, Clone)]
pub struct Credential {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub username: Option<String>,
    pub keyring_key: String,
    /// `true` for a reusable "vault" entry that can be referenced by many
    /// sessions and isn't deleted alongside any single session.
    pub reusable: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Snippet {
    pub id: i64,
    pub name: String,
    pub content: String,
    pub sort_order: i64,
    pub created_at: String,
    /// `None` = global (visible in every session). `Some(p)` = scoped to a
    /// single platform (matching the `core_hints::Platform::as_str()` form).
    pub platform: Option<String>,
    /// Optional accent color (hex) for the button-bar tile. `None` = default
    /// theme tint.
    pub color: Option<String>,
    /// `None` = a plain-text snippet (sends `content`). `Some(json)` = a macro:
    /// an expect/send/wait step array (same shape as `login_script_json`) run
    /// on the focused session.
    pub steps_json: Option<String>,
    /// Optional folder name for the sidebar Macros library. `None` = ungrouped.
    pub folder: Option<String>,
    /// Optional rank within the macro's folder group in the sidebar (NULLs
    /// last). Reordered independently of `sort_order`. `None` = unranked.
    pub position: Option<i64>,
    /// Optional keyboard shortcut (e.g. `"Ctrl+Alt+P"`) that fires this
    /// snippet or macro on the focused session. `None` = no shortcut.
    pub shortcut: Option<String>,
}

/// One row from `command_history`, used by the hint engine for
/// frecency-scored suggestions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandHistoryEntry {
    pub command: String,
    pub freq: i64,
    pub last_used: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HighlightRule {
    pub id: i64,
    pub name: String,
    pub pattern: String,
    pub is_regex: bool,
    pub fg_color: Option<String>,
    pub bg_color: Option<String>,
    pub bold: bool,
    pub underline: bool,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedSession {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub credential_id: Option<i64>,
    pub options_json: String,
    pub last_used_at: Option<String>,
    /// SSH auth method: NULL/"password" | "key" | "agent".
    pub auth_method: Option<String>,
    /// Optional ProxyJump: another saved session whose live SSH connection is
    /// used as the transport for this session (via a `direct-tcpip` channel).
    pub via_session_id: Option<i64>,
    /// Optional JSON array of `{expect, send, is_regex, timeout_ms}` login
    /// steps run automatically when the session is opened. NULL = no automation.
    pub login_script_json: Option<String>,
    /// Sort order inside the row's parent (folder or root). Migration 011
    /// seeded existing rows with their `id` so the previous "newest first"
    /// order is roughly preserved until the user re-arranges.
    pub sort_order: i64,
}

// ---------------------------------------------------------------------------
// Database handle
// ---------------------------------------------------------------------------

// rusqlite::Connection is Send; Mutex<Connection> is therefore Send + Sync.
/// Highest schema version this build knows (the last migration file).
pub const LATEST_SCHEMA: i64 = 26;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open (or create) the SQLite database at `path` and apply migrations.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let conn = Connection::open(path)?;
        // Enable WAL for better concurrent reads.
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    /// Write a clean, single-file snapshot of the live database to `dest`
    /// via `VACUUM INTO` — safe while the connection is open (no WAL/SHM
    /// sidecar files to copy). `dest` must not already exist.
    pub fn vacuum_into(&self, dest: &Path) -> Result<(), Error> {
        // SQLite takes the path as text: a path that isn't valid UTF-8 would
        // be rewritten (U+FFFD) and the snapshot land in another file.
        let dest = dest.to_str().ok_or_else(|| {
            Error::InvalidPath(format!("{} is not a valid UTF-8 path", dest.display()))
        })?;
        let conn = self.conn.lock().unwrap();
        conn.execute("VACUUM INTO ?1", rusqlite::params![dest])?;
        Ok(())
    }

    /// Snapshot the database file at `src` into `dest` (which must not
    /// exist) without changing `src`: opened read-only, copied with
    /// `VACUUM INTO`, so committed WAL contents are included. Used to import
    /// a ZAPX database on ZapTerm's first run.
    pub fn snapshot_file(src: &Path, dest: &Path) -> Result<(), Error> {
        let dest = dest.to_str().ok_or_else(|| {
            Error::InvalidPath(format!("{} is not a valid UTF-8 path", dest.display()))
        })?;
        let conn = Connection::open_with_flags(src, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.execute("VACUUM INTO ?1", rusqlite::params![dest])?;
        Ok(())
    }

    fn run_migrations(&self) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        // A fresh database has no schema_version table yet (version 0); on an
        // existing one a failing query is a real error, not "start over" —
        // re-running 001 on a populated database would fail confusingly.
        let has_table: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_version')",
            [],
            |row| row.get(0),
        )?;
        let version: i64 = if has_table {
            conn.query_row(
                "SELECT COALESCE(MAX(version),0) FROM schema_version",
                [],
                |row| row.get(0),
            )?
        } else {
            0
        };
        if version > LATEST_SCHEMA {
            return Err(Error::NewerSchema {
                found: version,
                supported: LATEST_SCHEMA,
            });
        }

        /// Run migration `n` if the database is below it, inside a
        /// transaction: an interruption (crash, power loss, full disk) rolls
        /// it back whole, instead of leaving half a migration that the next
        /// start can't get past ("duplicate column").
        fn apply(
            conn: &rusqlite::Connection,
            version: i64,
            n: i64,
            sql: &str,
        ) -> Result<(), Error> {
            if version < n {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(sql)?;
                tx.commit()?;
            }
            Ok(())
        }

        apply(
            &conn,
            version,
            1,
            include_str!("migrations/001_initial.sql"),
        )?;
        apply(
            &conn,
            version,
            2,
            include_str!("migrations/002_highlight_rules.sql"),
        )?;
        apply(
            &conn,
            version,
            3,
            include_str!("migrations/003_session_logs.sql"),
        )?;
        apply(
            &conn,
            version,
            4,
            include_str!("migrations/004_appearance.sql"),
        )?;
        apply(
            &conn,
            version,
            5,
            include_str!("migrations/005_ssh_key_auth.sql"),
        )?;
        apply(
            &conn,
            version,
            6,
            include_str!("migrations/006_jump_host.sql"),
        )?;
        apply(
            &conn,
            version,
            7,
            include_str!("migrations/007_snippets.sql"),
        )?;
        apply(
            &conn,
            version,
            8,
            include_str!("migrations/008_login_script.sql"),
        )?;
        apply(&conn, version, 9, include_str!("migrations/009_hints.sql"))?;
        apply(
            &conn,
            version,
            10,
            include_str!("migrations/010_session_secrets.sql"),
        )?;
        apply(
            &conn,
            version,
            11,
            include_str!("migrations/011_session_order.sql"),
        )?;
        apply(
            &conn,
            version,
            12,
            include_str!("migrations/012_snippet_platform.sql"),
        )?;
        apply(
            &conn,
            version,
            13,
            include_str!("migrations/013_broadcast_groups.sql"),
        )?;
        apply(
            &conn,
            version,
            14,
            include_str!("migrations/014_snippet_color.sql"),
        )?;
        apply(
            &conn,
            version,
            15,
            include_str!("migrations/015_session_forwards.sql"),
        )?;
        apply(
            &conn,
            version,
            16,
            include_str!("migrations/016_command_transitions.sql"),
        )?;
        apply(
            &conn,
            version,
            17,
            include_str!("migrations/017_session_triggers.sql"),
        )?;
        apply(
            &conn,
            version,
            18,
            include_str!("migrations/018_snippet_steps.sql"),
        )?;
        apply(
            &conn,
            version,
            19,
            include_str!("migrations/019_snippet_folder.sql"),
        )?;
        apply(
            &conn,
            version,
            20,
            include_str!("migrations/020_vault_entries.sql"),
        )?;
        apply(
            &conn,
            version,
            21,
            include_str!("migrations/021_vault_unique_name.sql"),
        )?;
        apply(
            &conn,
            version,
            22,
            include_str!("migrations/022_snippet_position.sql"),
        )?;
        apply(
            &conn,
            version,
            23,
            include_str!("migrations/023_credential_secrets.sql"),
        )?;
        apply(
            &conn,
            version,
            24,
            include_str!("migrations/024_snippet_shortcut.sql"),
        )?;
        apply(
            &conn,
            version,
            25,
            include_str!("migrations/025_forward_name.sql"),
        )?;
        apply(
            &conn,
            version,
            26,
            include_str!("migrations/026_richer_highlight_rules.sql"),
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Folder CRUD
    // -----------------------------------------------------------------------

    pub fn create_folder(&self, name: &str, parent_id: Option<i64>) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO folders (name, parent_id) VALUES (?1, ?2)",
            rusqlite::params![name, parent_id],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_folders(&self) -> Result<Vec<Folder>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, parent_id, name, sort_order FROM folders ORDER BY sort_order, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                name: row.get(2)?,
                sort_order: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn rename_folder(&self, id: i64, name: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE folders SET name=?1, updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![name, id],
        )?;
        Ok(())
    }

    pub fn delete_folder(&self, id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM folders WHERE id=?1", rusqlite::params![id])?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Broadcast group CRUD (multi-host orchestration)
    // -----------------------------------------------------------------------

    pub fn create_broadcast_group(&self, name: &str) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO broadcast_groups (name) VALUES (?1)",
            rusqlite::params![name],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// All groups with their ordered membership. Members are returned in their
    /// per-group `sort_order` so the grid lays them out predictably.
    pub fn list_broadcast_groups(&self) -> Result<Vec<BroadcastGroup>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, sort_order FROM broadcast_groups ORDER BY sort_order, name",
        )?;
        let groups = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut member_stmt = conn.prepare(
            "SELECT session_id FROM broadcast_group_members WHERE group_id=?1 ORDER BY sort_order",
        )?;
        let mut out = Vec::with_capacity(groups.len());
        for (id, name, sort_order) in groups {
            let session_ids = member_stmt
                .query_map(rusqlite::params![id], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            out.push(BroadcastGroup {
                id,
                name,
                sort_order,
                session_ids,
            });
        }
        Ok(out)
    }

    pub fn rename_broadcast_group(&self, id: i64, name: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE broadcast_groups SET name=?1, updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![name, id],
        )?;
        Ok(())
    }

    pub fn delete_broadcast_group(&self, id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        // Members cascade via the FK ON DELETE CASCADE.
        conn.execute(
            "DELETE FROM broadcast_groups WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    /// Replace a group's membership wholesale with `session_ids`, preserving
    /// the given order via `sort_order`. Idempotent — safe to call on every
    /// edit of the group in the UI.
    pub fn set_broadcast_group_members(
        &self,
        group_id: i64,
        session_ids: &[i64],
    ) -> Result<(), Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM broadcast_group_members WHERE group_id=?1",
            rusqlite::params![group_id],
        )?;
        for (idx, sid) in session_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO broadcast_group_members (group_id, session_id, sort_order)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![group_id, sid, idx as i64],
            )?;
        }
        tx.execute(
            "UPDATE broadcast_groups SET updated_at=datetime('now') WHERE id=?1",
            rusqlite::params![group_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Session CRUD
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn create_session(
        &self,
        folder_id: Option<i64>,
        name: &str,
        protocol: &str,
        host: Option<&str>,
        port: Option<u16>,
        username: Option<&str>,
        credential_id: Option<i64>,
        options_json: &str,
    ) -> Result<i64, Error> {
        self.create_session_full(
            folder_id,
            name,
            protocol,
            host,
            port,
            username,
            credential_id,
            options_json,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_session_full(
        &self,
        folder_id: Option<i64>,
        name: &str,
        protocol: &str,
        host: Option<&str>,
        port: Option<u16>,
        username: Option<&str>,
        credential_id: Option<i64>,
        options_json: &str,
        auth_method: Option<&str>,
        via_session_id: Option<i64>,
    ) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions
             (folder_id, name, protocol, host, port, username, credential_id, options_json, auth_method, via_session_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![
                folder_id,
                name,
                protocol,
                host,
                port.map(|p| p as i64),
                username,
                credential_id,
                options_json,
                auth_method,
                via_session_id,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_sessions(&self) -> Result<Vec<SavedSession>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, folder_id, name, protocol, host, port, username, credential_id,
                    options_json, last_used_at, auth_method, via_session_id, login_script_json,
                    sort_order
             FROM sessions
             ORDER BY folder_id IS NULL DESC, folder_id, sort_order, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SavedSession {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                name: row.get(2)?,
                protocol: row.get(3)?,
                host: row.get(4)?,
                port: row.get::<_, Option<i64>>(5)?.map(|p| p as u16),
                username: row.get(6)?,
                credential_id: row.get(7)?,
                options_json: row.get(8)?,
                last_used_at: row.get(9)?,
                auth_method: row.get(10)?,
                via_session_id: row.get(11)?,
                login_script_json: row.get(12)?,
                sort_order: row.get(13)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_session(&self, id: i64) -> Result<SavedSession, Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, folder_id, name, protocol, host, port, username, credential_id,
                    options_json, last_used_at, auth_method, via_session_id, login_script_json,
                    sort_order
             FROM sessions WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(SavedSession {
                    id: row.get(0)?,
                    folder_id: row.get(1)?,
                    name: row.get(2)?,
                    protocol: row.get(3)?,
                    host: row.get(4)?,
                    port: row.get::<_, Option<i64>>(5)?.map(|p| p as u16),
                    username: row.get(6)?,
                    credential_id: row.get(7)?,
                    options_json: row.get(8)?,
                    last_used_at: row.get(9)?,
                    auth_method: row.get(10)?,
                    via_session_id: row.get(11)?,
                    login_script_json: row.get(12)?,
                    sort_order: row.get(13)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
            e => Error::Rusqlite(e),
        })
    }

    /// Reparent a session to a folder (or `None` for the root). Used by the
    /// session-tree drag-and-drop.
    pub fn set_session_folder(&self, id: i64, folder_id: Option<i64>) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE sessions SET folder_id = ?1 WHERE id = ?2",
            rusqlite::params![folder_id, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Move `id` to position `target_index` inside `folder_id` (None = root).
    /// All rows currently in that container are re-numbered 0..N-1 so the
    /// next drag has stable, predictable inputs.
    ///
    /// Also updates `folder_id` if it differs from the row's current value —
    /// that way a single call handles both "drop into folder" and "reorder
    /// within folder" without needing the caller to issue two updates.
    pub fn reorder_session(
        &self,
        id: i64,
        folder_id: Option<i64>,
        target_index: usize,
    ) -> Result<(), Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        // Verify the row exists and pull its current container so we know
        // whether this is also a folder change.
        let current_folder: Option<i64> = tx
            .query_row(
                "SELECT folder_id FROM sessions WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                e => Error::Rusqlite(e),
            })?;

        // Collect every other row currently in the destination, ordered by
        // existing sort_order. We rebuild the order around the inserted row.
        let mut peers: Vec<i64> = Vec::new();
        match folder_id {
            Some(fid) => {
                let mut stmt = tx.prepare(
                    "SELECT id FROM sessions WHERE folder_id = ?1 AND id != ?2
                     ORDER BY sort_order, name",
                )?;
                let rows = stmt.query_map(rusqlite::params![fid, id], |r| r.get::<_, i64>(0))?;
                for row in rows {
                    peers.push(row?);
                }
            }
            None => {
                let mut stmt = tx.prepare(
                    "SELECT id FROM sessions WHERE folder_id IS NULL AND id != ?1
                     ORDER BY sort_order, name",
                )?;
                let rows = stmt.query_map(rusqlite::params![id], |r| r.get::<_, i64>(0))?;
                for row in rows {
                    peers.push(row?);
                }
            }
        }
        let clamped = target_index.min(peers.len());
        peers.insert(clamped, id);

        // Apply folder change (if any) before renumbering — keeps the
        // sort_order writes deterministic.
        if current_folder != folder_id {
            tx.execute(
                "UPDATE sessions SET folder_id = ?1 WHERE id = ?2",
                rusqlite::params![folder_id, id],
            )?;
        }

        for (i, rid) in peers.iter().enumerate() {
            tx.execute(
                "UPDATE sessions SET sort_order = ?1 WHERE id = ?2",
                rusqlite::params![i as i64, rid],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Update the user-editable fields of a saved session. Auth method and
    /// credential reference are intentionally NOT updated here — to change
    /// credentials the user re-creates the session in v1.
    #[allow(clippy::too_many_arguments)]
    pub fn update_session_fields(
        &self,
        id: i64,
        name: &str,
        folder_id: Option<i64>,
        host: Option<&str>,
        port: Option<u16>,
        username: Option<&str>,
        via_session_id: Option<i64>,
        options_json: Option<&str>,
    ) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE sessions
             SET name = ?1,
                 folder_id = ?2,
                 host = ?3,
                 port = ?4,
                 username = ?5,
                 via_session_id = ?6,
                 options_json = COALESCE(?7, options_json)
             WHERE id = ?8",
            rusqlite::params![
                name,
                folder_id,
                host,
                port.map(|p| p as i64),
                username,
                via_session_id,
                options_json,
                id,
            ],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Re-point a session's authentication: its `credential_id` (a private
    /// credential or a reusable vault entry, or `None` for agent/keyboard-
    /// interactive) and its `auth_method`. Unlike [`update_session_fields`],
    /// this is the *only* path that mutates the credential columns, so the
    /// caller owns keyring/secret lifecycle around it. Returns `NotFound` when
    /// no session has that id.
    pub fn set_session_auth(
        &self,
        id: i64,
        credential_id: Option<i64>,
        auth_method: &str,
    ) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE sessions SET credential_id = ?1, auth_method = ?2 WHERE id = ?3",
            rusqlite::params![credential_id, auth_method, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// List the port-forwards saved on a session, in display order.
    pub fn list_session_forwards(&self, session_id: i64) -> Result<Vec<SavedForward>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT kind, bind_addr, bind_port, target_host, target_port, name
             FROM session_forwards WHERE session_id = ?1 ORDER BY sort_order, id",
        )?;
        let rows = stmt.query_map(rusqlite::params![session_id], |row| {
            Ok(SavedForward {
                kind: row.get(0)?,
                bind_addr: row.get(1)?,
                bind_port: row.get::<_, i64>(2)? as u16,
                target_host: row.get(3)?,
                target_port: row.get::<_, Option<i64>>(4)?.map(|p| p as u16),
                name: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Replace the saved port-forwards of a session (delete-then-insert).
    pub fn set_session_forwards(
        &self,
        session_id: i64,
        forwards: &[SavedForward],
    ) -> Result<(), Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM session_forwards WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        for (idx, f) in forwards.iter().enumerate() {
            tx.execute(
                "INSERT INTO session_forwards
                 (session_id, kind, bind_addr, bind_port, target_host, target_port, sort_order, name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    session_id,
                    f.kind,
                    f.bind_addr,
                    f.bind_port as i64,
                    f.target_host,
                    f.target_port.map(|p| p as i64),
                    idx as i64,
                    f.name.as_deref().map(str::trim).filter(|n| !n.is_empty()),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Set (or clear) the ProxyJump bastion of a session. Used by the import
    /// pipeline's second pass, after every session id has been remapped.
    pub fn set_session_via(&self, id: i64, via_session_id: Option<i64>) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE sessions SET via_session_id = ?1 WHERE id = ?2",
            rusqlite::params![via_session_id, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn delete_session(&self, id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE id=?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn touch_session(&self, id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET last_used_at=datetime('now') WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Session secrets (encrypted local-fallback password cache)
    // -----------------------------------------------------------------------

    pub fn set_session_secret(&self, session_id: i64, ciphertext: &[u8]) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO session_secrets (session_id, ciphertext) VALUES (?1, ?2)
             ON CONFLICT(session_id) DO UPDATE SET
                 ciphertext = excluded.ciphertext,
                 updated_at = datetime('now')",
            rusqlite::params![session_id, ciphertext],
        )?;
        Ok(())
    }

    pub fn get_session_secret(&self, session_id: i64) -> Result<Option<Vec<u8>>, Error> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT ciphertext FROM session_secrets WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| row.get::<_, Vec<u8>>(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete_session_secret(&self, session_id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM session_secrets WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Credential secrets (encrypted local-fallback for vault entries)
    //
    // Mirrors `session_secrets`: the keyring stays the primary store, this is
    // consulted only when keyring retrieval fails (unsigned builds). The blob
    // is AES-256-GCM under `vault_seed`; the plaintext never leaves the
    // decrypt path in the vault send commands.
    // -----------------------------------------------------------------------

    pub fn set_credential_secret(
        &self,
        credential_id: i64,
        ciphertext: &[u8],
    ) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO credential_secrets (credential_id, ciphertext) VALUES (?1, ?2)
             ON CONFLICT(credential_id) DO UPDATE SET
                 ciphertext = excluded.ciphertext",
            rusqlite::params![credential_id, ciphertext],
        )?;
        Ok(())
    }

    pub fn get_credential_secret(&self, credential_id: i64) -> Result<Option<Vec<u8>>, Error> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT ciphertext FROM credential_secrets WHERE credential_id = ?1",
            rusqlite::params![credential_id],
            |row| row.get::<_, Vec<u8>>(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete_credential_secret(&self, credential_id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM credential_secrets WHERE credential_id = ?1",
            rusqlite::params![credential_id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Credential CRUD (keyring references only — no secrets in SQLite)
    // -----------------------------------------------------------------------

    pub fn create_credential(
        &self,
        name: &str,
        kind: &str,
        username: Option<&str>,
        keyring_key: &str,
    ) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO credentials (name, kind, username, keyring_key)
             VALUES (?1,?2,?3,?4)",
            rusqlite::params![name, kind, username, keyring_key],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Fetch a full credential row (metadata + keyring key, never the secret).
    /// Used when duplicating a credential for a cloned session.
    pub fn get_credential(&self, id: i64) -> Result<Credential, Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, kind, username, keyring_key, reusable FROM credentials WHERE id=?1",
            rusqlite::params![id],
            |row| {
                Ok(Credential {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    username: row.get(3)?,
                    keyring_key: row.get(4)?,
                    reusable: row.get::<_, i64>(5)? != 0,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
            e => Error::Rusqlite(e),
        })
    }

    /// All reusable "vault" credential entries (metadata + keyring key only).
    pub fn list_vault_entries(&self) -> Result<Vec<Credential>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, username, keyring_key, reusable
             FROM credentials WHERE reusable=1 ORDER BY name, id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Credential {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                username: row.get(3)?,
                keyring_key: row.get(4)?,
                reusable: row.get::<_, i64>(5)? != 0,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Fetch a reusable vault credential by its (unique) name. Returns
    /// `Ok(None)` when no reusable entry has that name. Used to resolve
    /// `{{vault:<Name>.<Field>}}` macro references at send time.
    pub fn get_credential_by_name(&self, name: &str) -> Result<Option<Credential>, Error> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT id, name, kind, username, keyring_key, reusable
             FROM credentials WHERE reusable=1 AND name=?1",
            rusqlite::params![name],
            |row| {
                Ok(Credential {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    username: row.get(3)?,
                    keyring_key: row.get(4)?,
                    reusable: row.get::<_, i64>(5)? != 0,
                })
            },
        );
        match result {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// True iff a reusable vault entry named `name` already exists, ignoring
    /// the row `exclude_id` (used so renaming an entry to its own name passes).
    /// Backs the friendly duplicate-name pre-check in the vault commands.
    pub fn vault_name_taken(&self, name: &str, exclude_id: Option<i64>) -> Result<bool, Error> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM credentials
             WHERE reusable=1 AND name=?1 AND id IS NOT ?2",
            rusqlite::params![name, exclude_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Create a reusable vault credential entry. The secret itself lives in the
    /// keyring under `keyring_key`; only the reference is stored here.
    pub fn create_vault_entry(
        &self,
        name: &str,
        username: Option<&str>,
        keyring_key: &str,
    ) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO credentials (name, kind, username, keyring_key, reusable)
             VALUES (?1, 'vault', ?2, ?3, 1)",
            rusqlite::params![name, username, keyring_key],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Update the editable metadata (name + username) of a credential row.
    /// The secret and keyring reference are left untouched.
    pub fn update_credential_meta(
        &self,
        id: i64,
        name: &str,
        username: Option<&str>,
    ) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "UPDATE credentials SET name=?2, username=?3 WHERE id=?1",
            rusqlite::params![id, name, username],
        )?;
        // Deleted meanwhile (another window): say so instead of "saved".
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn get_credential_keyring_key(&self, id: i64) -> Result<String, Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT keyring_key FROM credentials WHERE id=?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
            e => Error::Rusqlite(e),
        })
    }

    pub fn delete_credential(&self, id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM credentials WHERE id=?1", rusqlite::params![id])?;
        Ok(())
    }

    /// All `keyring_key` values currently referenced by `credentials`.
    pub fn list_credential_keyring_keys(&self) -> Result<Vec<String>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT keyring_key FROM credentials")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // -----------------------------------------------------------------------
    // Highlight rule CRUD
    // -----------------------------------------------------------------------

    pub fn list_highlight_rules(&self) -> Result<Vec<HighlightRule>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, pattern, is_regex, fg_color, bg_color,
                    bold, underline, enabled, sort_order
             FROM highlight_rules ORDER BY sort_order, id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(HighlightRule {
                id: row.get(0)?,
                name: row.get(1)?,
                pattern: row.get(2)?,
                is_regex: row.get::<_, i64>(3)? != 0,
                fg_color: row.get(4)?,
                bg_color: row.get(5)?,
                bold: row.get::<_, i64>(6)? != 0,
                underline: row.get::<_, i64>(7)? != 0,
                enabled: row.get::<_, i64>(8)? != 0,
                sort_order: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_highlight_rule(
        &self,
        name: &str,
        pattern: &str,
        is_regex: bool,
        fg_color: Option<&str>,
        bg_color: Option<&str>,
        bold: bool,
        underline: bool,
    ) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();
        let sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order),0)+1 FROM highlight_rules",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO highlight_rules
             (name, pattern, is_regex, fg_color, bg_color, bold, underline, sort_order)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![
                name,
                pattern,
                is_regex as i64,
                fg_color,
                bg_color,
                bold as i64,
                underline as i64,
                sort_order
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Overwrite a rule's editable fields (its identity — pattern + regex
    /// flag — stays). Used when a sync pull brings another device's edit.
    #[allow(clippy::too_many_arguments)]
    pub fn update_highlight_rule(
        &self,
        id: i64,
        name: &str,
        fg_color: Option<&str>,
        bg_color: Option<&str>,
        bold: bool,
        underline: bool,
        enabled: bool,
    ) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE highlight_rules
             SET name=?1, fg_color=?2, bg_color=?3, bold=?4, underline=?5, enabled=?6
             WHERE id=?7",
            rusqlite::params![
                name,
                fg_color,
                bg_color,
                bold as i64,
                underline as i64,
                enabled as i64,
                id
            ],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn toggle_highlight_rule(&self, id: i64, enabled: bool) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE highlight_rules SET enabled=?1 WHERE id=?2",
            rusqlite::params![enabled as i64, id],
        )?;
        Ok(())
    }

    pub fn delete_highlight_rule(&self, id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM highlight_rules WHERE id=?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Session log CRUD
    // -----------------------------------------------------------------------

    pub fn create_session_log(
        &self,
        session_id: Option<i64>,
        file_path: &str,
        started_at: &str,
    ) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO session_logs (session_id, file_path, started_at) VALUES (?1,?2,?3)",
            rusqlite::params![session_id, file_path, started_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn end_session_log(&self, id: i64, ended_at: &str, bytes: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE session_logs SET ended_at=?1, bytes=?2 WHERE id=?3",
            rusqlite::params![ended_at, bytes, id],
        )?;
        Ok(())
    }

    pub fn list_session_logs(&self, session_id: i64) -> Result<Vec<SessionLog>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, started_at, ended_at, file_path, bytes, format
             FROM session_logs WHERE session_id=?1 ORDER BY started_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![session_id], |row| {
            Ok(SessionLog {
                id: row.get(0)?,
                session_id: row.get(1)?,
                started_at: row.get(2)?,
                ended_at: row.get(3)?,
                file_path: row.get(4)?,
                bytes: row.get(5)?,
                format: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn list_all_session_logs(&self) -> Result<Vec<SessionLog>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, started_at, ended_at, file_path, bytes, format
             FROM session_logs ORDER BY started_at DESC LIMIT 200",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SessionLog {
                id: row.get(0)?,
                session_id: row.get(1)?,
                started_at: row.get(2)?,
                ended_at: row.get(3)?,
                file_path: row.get(4)?,
                bytes: row.get(5)?,
                format: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // -----------------------------------------------------------------------
    // Color scheme CRUD
    // -----------------------------------------------------------------------

    pub fn list_color_schemes(&self) -> Result<Vec<ColorScheme>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, palette_json, is_builtin FROM color_schemes ORDER BY is_builtin DESC, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ColorScheme {
                id: row.get(0)?,
                name: row.get(1)?,
                palette_json: row.get(2)?,
                is_builtin: row.get::<_, i64>(3)? != 0,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // -----------------------------------------------------------------------
    // Settings CRUD
    // -----------------------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, Error> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM settings WHERE key=?1",
            rusqlite::params![key],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    /// Remove a setting. A missing key is not an error.
    pub fn delete_setting(&self, key: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM settings WHERE key=?1", rusqlite::params![key])?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map = std::collections::HashMap::new();
        for r in rows {
            let (k, v) = r?;
            map.insert(k, v);
        }
        Ok(map)
    }

    // -----------------------------------------------------------------------
    // Login script (migration 008) — JSON blob on `sessions`.
    // -----------------------------------------------------------------------

    pub fn set_login_script(
        &self,
        session_id: i64,
        script_json: Option<&str>,
    ) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE sessions SET login_script_json = ?1 WHERE id = ?2",
            rusqlite::params![script_json, session_id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn get_login_script(&self, session_id: i64) -> Result<Option<String>, Error> {
        Ok(self.get_session(session_id)?.login_script_json)
    }

    /// Output triggers, stored as a JSON array on the session. Kept as dedicated
    /// queries (not part of the SavedSession SELECT) so the column is additive.
    pub fn set_session_triggers(
        &self,
        session_id: i64,
        triggers_json: Option<&str>,
    ) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE sessions SET triggers_json = ?1 WHERE id = ?2",
            rusqlite::params![triggers_json, session_id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn get_session_triggers(&self, session_id: i64) -> Result<Option<String>, Error> {
        let conn = self.conn.lock().unwrap();
        let json: Option<String> = conn
            .query_row(
                "SELECT triggers_json FROM sessions WHERE id = ?1",
                rusqlite::params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => Error::from(other),
            })?;
        Ok(json)
    }

    // -----------------------------------------------------------------------
    // Snippets CRUD (migration 007)
    // -----------------------------------------------------------------------

    pub fn list_snippets(&self) -> Result<Vec<Snippet>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, content, sort_order, created_at, platform, color, steps_json, folder, position, shortcut
             FROM snippets ORDER BY sort_order, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Snippet {
                id: row.get(0)?,
                name: row.get(1)?,
                content: row.get(2)?,
                sort_order: row.get(3)?,
                created_at: row.get(4)?,
                platform: row.get(5)?,
                color: row.get(6)?,
                steps_json: row.get(7)?,
                folder: row.get(8)?,
                position: row.get(9)?,
                shortcut: row.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Same as [`list_snippets`] but filtered to snippets visible from a
    /// session of `platform` — globals (NULL) plus matches. Globals first,
    /// then platform-specific.
    pub fn list_snippets_for_platform(&self, platform: &str) -> Result<Vec<Snippet>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, content, sort_order, created_at, platform, color, steps_json, folder, position, shortcut
             FROM snippets
             WHERE platform IS NULL OR platform = ?1
             ORDER BY (platform IS NULL) DESC, sort_order, name",
        )?;
        let rows = stmt.query_map(rusqlite::params![platform], |row| {
            Ok(Snippet {
                id: row.get(0)?,
                name: row.get(1)?,
                content: row.get(2)?,
                sort_order: row.get(3)?,
                created_at: row.get(4)?,
                platform: row.get(5)?,
                color: row.get(6)?,
                steps_json: row.get(7)?,
                folder: row.get(8)?,
                position: row.get(9)?,
                shortcut: row.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// True iff there is at least one snippet tagged with `platform`. Used
    /// by the app layer to gate first-time-seen seeding of platform defaults.
    pub fn snippets_exist_for_platform(&self, platform: &str) -> Result<bool, Error> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM snippets WHERE platform = ?1",
            rusqlite::params![platform],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    pub fn create_snippet(
        &self,
        name: &str,
        content: &str,
        platform: Option<&str>,
        color: Option<&str>,
    ) -> Result<i64, Error> {
        let conn = self.conn.lock().unwrap();
        // Next sort_order = current max + 1; keeps user-created entries after seeds.
        let next: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM snippets",
                [],
                |r| r.get(0),
            )
            .unwrap_or(1);
        conn.execute(
            "INSERT INTO snippets (name, content, sort_order, platform, color)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![name, content, next, platform, color],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_snippet(
        &self,
        id: i64,
        name: &str,
        content: &str,
        platform: Option<&str>,
        color: Option<&str>,
    ) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE snippets SET name = ?1, content = ?2, platform = ?3, color = ?4 WHERE id = ?5",
            rusqlite::params![name, content, platform, color, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Set (or clear with `None`) a snippet's macro steps. Kept separate from
    /// [`update_snippet`] so the existing create/update call sites are unchanged
    /// (mirrors how login scripts are stored).
    pub fn set_snippet_steps(&self, id: i64, steps_json: Option<&str>) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE snippets SET steps_json = ?1 WHERE id = ?2",
            rusqlite::params![steps_json, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Set (or clear with `None`) a snippet's folder. Kept separate from
    /// `update_snippet` so existing call sites are unchanged (mirrors
    /// [`set_snippet_steps`]).
    ///
    /// Moving to a DIFFERENT folder resets `position` to NULL so the macro
    /// lands at the BOTTOM of the destination group (NULLs sort last) rather
    /// than carrying its old rank — e.g. an old position 0 jumping it to the
    /// top of the new folder. Re-setting the same folder leaves `position`
    /// intact. Safe for the bottom bar, which orders by `sort_order`, not
    /// `position`.
    pub fn set_snippet_folder(&self, id: i64, folder: Option<&str>) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let current: Option<String> = conn
            .query_row(
                "SELECT folder FROM snippets WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                e => Error::Rusqlite(e),
            })?;
        let folder_changed = current.as_deref() != folder;
        let n = if folder_changed {
            conn.execute(
                "UPDATE snippets SET folder = ?1, position = NULL WHERE id = ?2",
                rusqlite::params![folder, id],
            )?
        } else {
            conn.execute(
                "UPDATE snippets SET folder = ?1 WHERE id = ?2",
                rusqlite::params![folder, id],
            )?
        };
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Set (or clear with `None`) a snippet's keyboard shortcut. Kept separate
    /// from [`update_snippet`] like the other optional fields; a blank combo
    /// is stored as NULL.
    pub fn set_snippet_shortcut(&self, id: i64, shortcut: Option<&str>) -> Result<(), Error> {
        let shortcut = shortcut.map(str::trim).filter(|s| !s.is_empty());
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE snippets SET shortcut = ?1 WHERE id = ?2",
            rusqlite::params![shortcut, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Set (or clear) one macro's rank inside its folder, as carried by an
    /// export/sync bundle.
    pub fn set_snippet_position(&self, id: i64, position: Option<i64>) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE snippets SET position = ?1 WHERE id = ?2",
            rusqlite::params![position, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Persist a new per-folder ordering for macros: each id's `position`
    /// becomes its index in `ordered_ids`. Used by the sidebar Macros zone's
    /// ▲/▼ reorder. Ids not listed keep their position (NULL or prior rank).
    pub fn set_snippet_positions(&self, ordered_ids: &[i64]) -> Result<(), Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (idx, id) in ordered_ids.iter().enumerate() {
            tx.execute(
                "UPDATE snippets SET position = ?1 WHERE id = ?2",
                rusqlite::params![idx as i64, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_snippet(&self, id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM snippets WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    /// Persist a new display order for the given snippet ids: `sort_order`
    /// becomes each id's position in the list. Ids not listed keep theirs.
    pub fn set_snippets_order(&self, ordered_ids: &[i64]) -> Result<(), Error> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (idx, id) in ordered_ids.iter().enumerate() {
            tx.execute(
                "UPDATE snippets SET sort_order = ?1 WHERE id = ?2",
                rusqlite::params![idx as i64, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Command history (powers the ghost-text / popup hints)
    // -----------------------------------------------------------------------

    /// Insert-or-bump: if (session_id, command) already exists, increment
    /// freq and refresh last_used. Otherwise insert a new row.
    pub fn record_command(&self, session_id: Option<i64>, command: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute(
            "UPDATE command_history
                SET freq = freq + 1, last_used = datetime('now')
              WHERE command = ?1
                AND ((?2 IS NULL AND session_id IS NULL)
                  OR session_id = ?2)",
            rusqlite::params![command, session_id],
        )?;
        if updated == 0 {
            conn.execute(
                "INSERT INTO command_history (session_id, command) VALUES (?1, ?2)",
                rusqlite::params![session_id, command],
            )?;
        }
        Ok(())
    }

    /// Return history entries matching `prefix` (case-insensitive). Pulls
    /// both session-scoped and global rows so generic commands stay useful
    /// in fresh sessions.
    pub fn history_by_prefix(
        &self,
        session_id: Option<i64>,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<CommandHistoryEntry>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT command, freq, last_used FROM command_history
              WHERE (session_id = ?1 OR session_id IS NULL)
                AND command LIKE ?2 || '%' ESCAPE '\\'
              ORDER BY last_used DESC
              LIMIT ?3",
        )?;
        let escaped = escape_like(prefix);
        let rows = stmt.query_map(
            rusqlite::params![session_id, escaped, limit as i64],
            |row| {
                Ok(CommandHistoryEntry {
                    command: row.get(0)?,
                    freq: row.get(1)?,
                    last_used: row.get(2)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Substring search over command history for the Ctrl+R reverse-i-search:
    /// scoped to this session (plus session-less entries), most-recent first.
    /// An empty `query` returns the most recent commands. Commands are already
    /// de-duplicated in `command_history` (one row per command).
    pub fn search_history(
        &self,
        session_id: Option<i64>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT command FROM command_history
              WHERE (session_id = ?1 OR session_id IS NULL)
                AND command LIKE '%' || ?2 || '%' ESCAPE '\\'
              ORDER BY last_used DESC
              LIMIT ?3",
        )?;
        let escaped = escape_like(query);
        let rows = stmt.query_map(
            rusqlite::params![session_id, escaped, limit as i64],
            |row| row.get::<_, String>(0),
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Top `limit` commands for the dynamic "Recents" snippet bar. Ranked
    /// by frecency: a freshly-used command outranks an old frequent one,
    /// but raw frequency still matters within the same age band.
    ///
    /// We hide short / trivial commands (≤ 2 chars) — they're noisy and
    /// don't earn their slot in the bar.
    /// The last commands run in one session — only that session's rows. With
    /// `None` (local shells, quick connects) only the rows with no session.
    /// Unlike [`Self::history_by_prefix`], rows of other sessions never mix
    /// in: this is the "Recent" menu of the focused session.
    pub fn recent_commands(
        &self,
        session_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CommandHistoryEntry>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT command, freq, last_used FROM command_history
              WHERE session_id IS ?1
                AND length(command) > 2
              ORDER BY last_used DESC, freq DESC
              LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![session_id, limit as i64], |row| {
            Ok(CommandHistoryEntry {
                command: row.get(0)?,
                freq: row.get(1)?,
                last_used: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn clear_history(&self, session_id: Option<i64>) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        match session_id {
            Some(id) => conn.execute(
                "DELETE FROM command_history WHERE session_id = ?1",
                rusqlite::params![id],
            )?,
            None => conn.execute("DELETE FROM command_history", [])?,
        };
        // Transitions are platform-keyed (not session-scoped); clear them
        // wholesale only on a global wipe.
        if session_id.is_none() {
            conn.execute("DELETE FROM command_transitions", [])?;
        }
        Ok(())
    }

    /// Record that `next` followed `prev` for `platform` (insert-or-bump).
    /// Used by the command-sequence learner.
    pub fn record_transition(&self, platform: &str, prev: &str, next: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute(
            "UPDATE command_transitions
                SET freq = freq + 1, last_used = datetime('now')
              WHERE platform = ?1 AND prev = ?2 AND next = ?3",
            rusqlite::params![platform, prev, next],
        )?;
        if updated == 0 {
            conn.execute(
                "INSERT INTO command_transitions (platform, prev, next) VALUES (?1, ?2, ?3)",
                rusqlite::params![platform, prev, next],
            )?;
        }
        Ok(())
    }

    /// Commands that frequently follow `prev` on `platform`, as
    /// `(next_command, freq)` ordered by frequency. Powers the next-command
    /// boost in the hint engine.
    pub fn next_commands_after(
        &self,
        platform: &str,
        prev: &str,
        limit: usize,
    ) -> Result<Vec<(String, i64)>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT next, freq FROM command_transitions
              WHERE platform = ?1 AND prev = ?2
              ORDER BY freq DESC, last_used DESC
              LIMIT ?3",
        )?;
        let rows = stmt.query_map(rusqlite::params![platform, prev, limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Read global snippets (managed by the dedicated snippets dialog).
    /// The hint engine surfaces these in the popup alongside history.
    pub fn list_snippet_contents_by_prefix(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<(String, String)>, Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, content FROM snippets
              WHERE content LIKE ?1 || '%' ESCAPE '\\'
              ORDER BY sort_order, name
              LIMIT ?2",
        )?;
        let escaped = escape_like(prefix);
        let rows = stmt.query_map(rusqlite::params![escaped, limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // -----------------------------------------------------------------------
    // Per-session platform override (for hint catalogs)
    // -----------------------------------------------------------------------

    pub fn set_session_platform(&self, session_id: i64, platform: &str) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO session_platforms (session_id, platform) VALUES (?1, ?2)
             ON CONFLICT(session_id) DO UPDATE SET
                 platform = excluded.platform,
                 updated_at = datetime('now')",
            rusqlite::params![session_id, platform],
        )?;
        Ok(())
    }

    /// Forget a session's platform: hints fall back to the global default and
    /// the next detection may set it again.
    pub fn clear_session_platform(&self, session_id: i64) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM session_platforms WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        Ok(())
    }

    pub fn get_session_platform(&self, session_id: i64) -> Result<Option<String>, Error> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT platform FROM session_platforms WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// Escape LIKE wildcards (%, _, \) in user input so a prefix containing a
/// literal % does not behave as a wildcard.
fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch == '\\' || ch == '%' || ch == '_' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open a throwaway DB in a unique temp path so migrations run fresh.
    fn temp_db() -> (Database, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("zapterm_groups_test_{nanos}.db"));
        let db = Database::open(&path).expect("open db");
        (db, path)
    }

    #[test]
    fn recent_commands_stay_in_their_session() {
        let (db, path) = temp_db();
        let mk = |name: &str| {
            db.create_session(
                None,
                name,
                "ssh",
                Some("10.0.0.1"),
                Some(22),
                None,
                None,
                "{}",
            )
            .unwrap()
        };
        let (a, b) = (mk("a"), mk("b"));
        db.record_command(Some(a), "show clock").unwrap();
        db.record_command(Some(b), "show version").unwrap();
        db.record_command(None, "sleep 30").unwrap();
        let texts = |sid| -> Vec<String> {
            db.recent_commands(sid, 10)
                .unwrap()
                .into_iter()
                .map(|e| e.command)
                .collect()
        };
        assert_eq!(texts(Some(a)), vec!["show clock"]);
        assert_eq!(texts(Some(b)), vec!["show version"]);
        assert_eq!(texts(None), vec!["sleep 30"]);
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_session_platform_can_be_cleared() {
        let (db, path) = temp_db();
        let sid = db
            .create_session(
                None,
                "fw",
                "ssh",
                Some("10.0.0.1"),
                Some(22),
                None,
                None,
                "{}",
            )
            .unwrap();
        db.set_session_platform(sid, "fortigate").unwrap();
        assert_eq!(
            db.get_session_platform(sid).unwrap().as_deref(),
            Some("fortigate")
        );
        db.clear_session_platform(sid).unwrap();
        assert_eq!(db.get_session_platform(sid).unwrap(), None);
        db.clear_session_platform(sid).unwrap(); // nothing stored: still fine
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn forward_names_are_saved_trimmed_and_optional() {
        let (db, path) = temp_db();
        let sid = db
            .create_session(
                None,
                "jump",
                "ssh",
                Some("10.0.0.1"),
                Some(22),
                None,
                None,
                "{}",
            )
            .unwrap();
        let fwd = |name: Option<&str>| SavedForward {
            kind: "local".into(),
            bind_addr: "127.0.0.1".into(),
            bind_port: 8443,
            target_host: Some("10.0.0.2".into()),
            target_port: Some(443),
            name: name.map(str::to_owned),
        };
        db.set_session_forwards(
            sid,
            &[fwd(Some("  FortiGate GUI ")), fwd(Some("   ")), fwd(None)],
        )
        .unwrap();
        let names: Vec<_> = db
            .list_session_forwards(sid)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, [Some("FortiGate GUI".to_owned()), None, None]);
        // Exports made before names existed still deserialize.
        let old: SavedForward = serde_json::from_str(
            r#"{"kind":"dynamic","bind_addr":"127.0.0.1","bind_port":1080,"target_host":null,"target_port":null}"#,
        )
        .unwrap();
        assert_eq!(old.name, None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn broadcast_group_crud_roundtrip() {
        let (db, path) = temp_db();

        // Seed two saved sessions to reference as members.
        let s1 = db
            .create_session(
                None,
                "r1",
                "ssh",
                Some("10.0.0.1"),
                Some(22),
                Some("admin"),
                None,
                "{}",
            )
            .unwrap();
        let s2 = db
            .create_session(
                None,
                "r2",
                "ssh",
                Some("10.0.0.2"),
                Some(22),
                Some("admin"),
                None,
                "{}",
            )
            .unwrap();

        // Create + set members (note reversed order is preserved).
        let gid = db.create_broadcast_group("core").unwrap();
        db.set_broadcast_group_members(gid, &[s2, s1]).unwrap();

        let groups = db.list_broadcast_groups().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "core");
        assert_eq!(groups[0].session_ids, vec![s2, s1]);

        // Rename.
        db.rename_broadcast_group(gid, "core-switches").unwrap();
        assert_eq!(db.list_broadcast_groups().unwrap()[0].name, "core-switches");

        // Deleting a referenced session cascades it out of the membership.
        db.delete_session(s2).unwrap();
        assert_eq!(db.list_broadcast_groups().unwrap()[0].session_ids, vec![s1]);

        // Deleting the group removes it (and members cascade).
        db.delete_broadcast_group(gid).unwrap();
        assert!(db.list_broadcast_groups().unwrap().is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_session_auth_repoints_credential_and_method() {
        let (db, path) = temp_db();

        // A password session that starts on a private credential.
        let cred = db
            .create_credential("r1", "ssh_password", Some("admin"), "ssh:key-1")
            .unwrap();
        let sid = db
            .create_session_full(
                None,
                "r1",
                "ssh",
                Some("10.0.0.1"),
                Some(22),
                Some("admin"),
                Some(cred),
                "{}",
                Some("password"),
                None,
            )
            .unwrap();
        let s = db.get_session(sid).unwrap();
        assert_eq!(s.credential_id, Some(cred));
        assert_eq!(s.auth_method.as_deref(), Some("password"));

        // Re-point at a different (vault) credential — still password auth.
        let vault = db
            .create_vault_entry("prod-root", Some("root"), "vault:key-1")
            .unwrap();
        db.set_session_auth(sid, Some(vault), "password").unwrap();
        let s = db.get_session(sid).unwrap();
        assert_eq!(s.credential_id, Some(vault));
        assert_eq!(s.auth_method.as_deref(), Some("password"));

        // Switch to a credential-less method (agent): credential_id clears.
        db.set_session_auth(sid, None, "agent").unwrap();
        let s = db.get_session(sid).unwrap();
        assert_eq!(s.credential_id, None);
        assert_eq!(s.auth_method.as_deref(), Some("agent"));

        // Unknown session id is a clean NotFound, not a silent no-op.
        assert!(matches!(
            db.set_session_auth(999_999, None, "agent"),
            Err(Error::NotFound)
        ));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn vault_entry_by_name_and_unique() {
        let (db, path) = temp_db();

        let id = db
            .create_vault_entry("prod-root", Some("root"), "vault:key-1")
            .unwrap();

        // Resolve by name returns the reusable row.
        let found = db.get_credential_by_name("prod-root").unwrap().unwrap();
        assert_eq!(found.id, id);
        assert_eq!(found.username.as_deref(), Some("root"));

        // Unknown name → None.
        assert!(db.get_credential_by_name("nope").unwrap().is_none());

        // A non-reusable credential of the same name is invisible to the lookup.
        db.create_credential("prod-root", "session", None, "k2")
            .unwrap();
        assert_eq!(
            db.get_credential_by_name("prod-root").unwrap().unwrap().id,
            id
        );

        // Duplicate-name pre-check: taken for others, free for itself.
        assert!(db.vault_name_taken("prod-root", None).unwrap());
        assert!(!db.vault_name_taken("prod-root", Some(id)).unwrap());
        assert!(!db.vault_name_taken("fresh", None).unwrap());

        // The partial unique index rejects a second reusable row with that name.
        assert!(db
            .create_vault_entry("prod-root", None, "vault:key-3")
            .is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn credential_secret_crud_roundtrip() {
        let (db, path) = temp_db();

        let id = db
            .create_vault_entry("prod-root", Some("root"), "vault:key-1")
            .unwrap();

        // Absent until written.
        assert!(db.get_credential_secret(id).unwrap().is_none());

        // Insert, then read back the exact bytes.
        let blob = b"\x01\x02ciphertext\xff".to_vec();
        db.set_credential_secret(id, &blob).unwrap();
        assert_eq!(db.get_credential_secret(id).unwrap(), Some(blob.clone()));

        // ON CONFLICT updates in place.
        let blob2 = b"rotated".to_vec();
        db.set_credential_secret(id, &blob2).unwrap();
        assert_eq!(db.get_credential_secret(id).unwrap(), Some(blob2));

        // Explicit delete clears it.
        db.delete_credential_secret(id).unwrap();
        assert!(db.get_credential_secret(id).unwrap().is_none());

        // FK ON DELETE CASCADE also removes the fallback when the row goes.
        db.set_credential_secret(id, &blob).unwrap();
        db.delete_credential(id).unwrap();
        assert!(db.get_credential_secret(id).unwrap().is_none());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn snippet_shortcut_set_and_clear() {
        let (db, path) = temp_db();
        let id = db.create_snippet("pw", "", None, None).unwrap();
        let get = |db: &Database| {
            db.list_snippets()
                .unwrap()
                .into_iter()
                .find(|s| s.id == id)
                .unwrap()
                .shortcut
        };
        assert_eq!(get(&db), None);
        db.set_snippet_shortcut(id, Some("Ctrl+Alt+P")).unwrap();
        assert_eq!(get(&db).as_deref(), Some("Ctrl+Alt+P"));
        // Also visible through the platform-filtered listing.
        let listed = db.list_snippets_for_platform("fortigate").unwrap();
        assert_eq!(
            listed
                .iter()
                .find(|s| s.id == id)
                .unwrap()
                .shortcut
                .as_deref(),
            Some("Ctrl+Alt+P")
        );
        // Blank clears.
        db.set_snippet_shortcut(id, Some("  ")).unwrap();
        assert_eq!(get(&db), None);
        assert!(matches!(
            db.set_snippet_shortcut(999_999, Some("Ctrl+Alt+X")),
            Err(Error::NotFound)
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn moving_macro_to_new_folder_resets_position() {
        let (db, path) = temp_db();

        let find = |db: &Database, id: i64| -> Snippet {
            db.list_snippets()
                .unwrap()
                .into_iter()
                .find(|s| s.id == id)
                .unwrap()
        };

        let id = db.create_snippet("m", "", None, None).unwrap();
        db.set_snippet_steps(id, Some("[]")).unwrap();
        db.set_snippet_folder(id, Some("A")).unwrap();
        // Rank it at the top of folder A.
        db.set_snippet_positions(&[id]).unwrap();
        assert_eq!(find(&db, id).position, Some(0));

        // Moving to a different folder clears the stale rank (NULLs sort last).
        db.set_snippet_folder(id, Some("B")).unwrap();
        let moved = find(&db, id);
        assert_eq!(moved.folder.as_deref(), Some("B"));
        assert_eq!(moved.position, None);

        // Re-ranking, then re-setting the SAME folder leaves position intact.
        db.set_snippet_positions(&[id]).unwrap();
        db.set_snippet_folder(id, Some("B")).unwrap();
        assert_eq!(find(&db, id).position, Some(0));

        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("zapterm-mig-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("zapterm.db")
    }

    /// A snapshot carries the source's rows (WAL included) and leaves it as is.
    #[test]
    fn snapshot_file_copies_without_touching_the_source() {
        let src = temp_db("snap-src");
        let dest = src.with_file_name("copy.db");
        let db = Database::open(&src).unwrap();
        db.set_setting("k", "v").unwrap();
        // Keep `db` open: the row may still be only in the WAL.
        Database::snapshot_file(&src, &dest).unwrap();
        let copy = Database::open(&dest).unwrap();
        assert_eq!(copy.get_setting("k").unwrap().as_deref(), Some("v"));
        assert!(Database::snapshot_file(&src, &dest).is_err(), "dest exists");
        drop(db);
    }

    /// A database from a newer ZapTerm is refused with a clear error instead of
    /// being used (and half-understood) by this one.
    #[test]
    fn a_newer_schema_is_refused() {
        let path = temp_db("newer");
        drop(Database::open(&path).unwrap());
        {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute(
                "INSERT INTO schema_version(version) VALUES (?1)",
                [LATEST_SCHEMA + 1],
            )
            .unwrap();
        }
        let err = Database::open(&path).err().expect("must be refused");
        assert!(err.to_string().contains("newer ZapTerm"), "{err}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// Reopening an up-to-date database runs nothing again.
    #[test]
    fn reopening_is_idempotent() {
        let path = temp_db("reopen");
        drop(Database::open(&path).unwrap());
        drop(Database::open(&path).unwrap());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 026 adds the richer highlight rules after the user's own, never twice,
    /// and replaces the old IPv6 rule only where the user left it as shipped.
    #[test]
    fn migration_026_adds_highlight_rules_once() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE schema_version(version INTEGER PRIMARY KEY);")
            .unwrap();
        c.execute_batch(include_str!("migrations/002_highlight_rules.sql"))
            .unwrap();
        // The user already has the Time rule (e.g. synced from another device)
        // and a rule of their own at the end.
        let time = r"\b(?:[01]\d|2[0-3]):[0-5]\d(?::[0-5]\d(?:[.,]\d{1,6})?)?\b";
        c.execute(
            "INSERT INTO highlight_rules(name, pattern, sort_order) VALUES ('my time', ?1, 50), ('mine', 'foo', 51)",
            [time],
        )
        .unwrap();
        let count = |c: &rusqlite::Connection| -> i64 {
            c.query_row("SELECT COUNT(*) FROM highlight_rules", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(count(&c), 12);

        c.execute_batch(include_str!("migrations/026_richer_highlight_rules.sql"))
            .unwrap();
        assert_eq!(count(&c), 23, "11 new rules: Time was already there");
        let ipv6: String = c
            .query_row(
                "SELECT pattern FROM highlight_rules WHERE name='IPv6 address'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(ipv6.contains("{7}"), "shipped IPv6 rule replaced: {ipv6}");
        let first_new: i64 = c
            .query_row(
                "SELECT MIN(sort_order) FROM highlight_rules WHERE name LIKE 'Usage%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            first_new > 51,
            "new rules come after the user's: {first_new}"
        );

        // Again (e.g. a restored backup): nothing more.
        c.execute_batch(include_str!("migrations/026_richer_highlight_rules.sql"))
            .unwrap();
        assert_eq!(count(&c), 23);

        // An IPv6 rule the user edited is left alone.
        let c2 = rusqlite::Connection::open_in_memory().unwrap();
        c2.execute_batch("CREATE TABLE schema_version(version INTEGER PRIMARY KEY);")
            .unwrap();
        c2.execute_batch(include_str!("migrations/002_highlight_rules.sql"))
            .unwrap();
        c2.execute(
            "UPDATE highlight_rules SET pattern='my-ipv6' WHERE name='IPv6 address'",
            [],
        )
        .unwrap();
        c2.execute_batch(include_str!("migrations/026_richer_highlight_rules.sql"))
            .unwrap();
        let kept: String = c2
            .query_row(
                "SELECT pattern FROM highlight_rules WHERE name='IPv6 address'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, "my-ipv6");
    }

    /// 0.13.0 allowed duplicate vault names; 021 renames the extras instead
    /// of failing (which used to block the upgrade for good).
    #[test]
    fn migration_021_renames_duplicate_vault_names() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE schema_version(version INTEGER PRIMARY KEY);
             CREATE TABLE credentials(id INTEGER PRIMARY KEY, name TEXT, reusable INTEGER);
             INSERT INTO credentials(id, name, reusable) VALUES
               (1, 'prod-root', 1), (2, 'prod-root', 1), (3, 'prod-root', 0), (4, 'lab', 1);",
        )
        .unwrap();
        c.execute_batch(include_str!("migrations/021_vault_unique_name.sql"))
            .unwrap();
        let names: Vec<String> = c
            .prepare("SELECT name FROM credentials ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(names, ["prod-root", "prod-root (2)", "prod-root", "lab"]);
    }
}
