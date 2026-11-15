//! Dictation history: one SQLite row per take, kept on this Mac only.
//!
//! Text and timings only. Audio is never written here.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

use crate::problem::Problem;
use crate::settings::HistoryRetention;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum TakeStatus {
    Pasted,
    Skipped,
    Failed,
}

impl TakeStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pasted => "pasted",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "pasted" => Ok(Self::Pasted),
            "skipped" => Ok(Self::Skipped),
            "failed" => Ok(Self::Failed),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown take status {other:?}").into(),
            )),
        }
    }
}

/// A finished take, before it has an id.
#[derive(Clone, Debug, PartialEq)]
pub struct NewTake {
    pub text: String,
    pub created_at_ms: i64,
    pub app: Option<String>,
    pub model: String,
    pub audio_ms: i64,
    pub inference_ms: Option<i64>,
    pub total_ms: i64,
    pub status: TakeStatus,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    #[ts(type = "number")]
    pub id: i64,
    pub text: String,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    pub app: Option<String>,
    pub model: String,
    #[ts(type = "number")]
    pub audio_ms: i64,
    #[ts(type = "number | null")]
    pub inference_ms: Option<i64>,
    #[ts(type = "number")]
    pub total_ms: i64,
    pub status: TakeStatus,
    pub detail: Option<String>,
}

/// One page of takes, newest first. `has_more` is true when older matching
/// takes exist beyond the requested limit.
#[derive(Clone, Debug, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistoryList {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

/// Each entry upgrades the schema by one `user_version`. Append, never edit.
const MIGRATIONS: &[&str] = &["CREATE TABLE takes (
        id INTEGER PRIMARY KEY,
        text TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL,
        app TEXT,
        model TEXT NOT NULL,
        audio_ms INTEGER NOT NULL,
        inference_ms INTEGER,
        total_ms INTEGER NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('pasted', 'skipped', 'failed')),
        detail TEXT
    );
    CREATE INDEX takes_created_at ON takes (created_at_ms);"];

/// Why history could not open. A newer schema needs a newer app; anything
/// else may clear up on restart.
#[derive(Debug, PartialEq)]
pub enum OpenError {
    NewerSchema { found: i64, supported: i64 },
    Failed(String),
}

impl OpenError {
    pub fn problem(&self) -> Problem {
        match self {
            Self::NewerSchema { found, supported } => Problem::new(
                "History is off",
                "This version of dictate can’t read your history. Update dictate to see it again.",
            )
            .because(format!(
                "history schema {found} is newer than this app ({supported})"
            )),
            Self::Failed(cause) => Problem::new(
                "History is off",
                "Dictation still works. Restart dictate to turn history back on.",
            )
            .because(cause),
        }
    }
}

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| OpenError::Failed(format!("Create history folder: {e}")))?;
        }
        let connection =
            Connection::open(path).map_err(|e| OpenError::Failed(format!("Open history: {e}")))?;
        Self::from_connection(connection)
    }

    #[cfg(test)]
    fn in_memory() -> Self {
        Self::from_connection(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn from_connection(mut connection: Connection) -> Result<Self, OpenError> {
        // Deleting a take promises its text can't be recovered: overwrite freed
        // pages instead of leaving them in the file.
        connection
            .pragma_update(None, "secure_delete", "ON")
            .map_err(|e| OpenError::Failed(format!("Turn on secure delete: {e}")))?;
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn insert(&self, take: &NewTake) -> Result<i64, String> {
        self.connection
            .execute(
                "INSERT INTO takes
                 (text, created_at_ms, app, model, audio_ms, inference_ms, total_ms, status, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    take.text,
                    take.created_at_ms,
                    take.app,
                    take.model,
                    take.audio_ms,
                    take.inference_ms,
                    take.total_ms,
                    take.status.as_str(),
                    take.detail,
                ],
            )
            .map_err(|e| format!("Save dictation: {e}"))?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Up to `limit` takes, newest first. `search` matches text or app name,
    /// case-insensitively.
    pub fn list(&self, search: &str, limit: u32) -> Result<HistoryList, String> {
        let pattern = format!("%{}%", escape_like(search.trim()));
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT id, text, created_at_ms, app, model, audio_ms, inference_ms, total_ms, status, detail
                 FROM takes
                 WHERE text LIKE ?1 ESCAPE '\\' OR app LIKE ?1 ESCAPE '\\'
                 ORDER BY created_at_ms DESC, id DESC
                 LIMIT ?2",
            )
            .map_err(|e| format!("Read history: {e}"))?;
        let mut entries = statement
            .query_map(params![pattern, i64::from(limit) + 1], |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    text: row.get(1)?,
                    created_at_ms: row.get(2)?,
                    app: row.get(3)?,
                    model: row.get(4)?,
                    audio_ms: row.get(5)?,
                    inference_ms: row.get(6)?,
                    total_ms: row.get(7)?,
                    status: TakeStatus::parse(&row.get::<_, String>(8)?)?,
                    detail: row.get(9)?,
                })
            })
            .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
            .map_err(|e| format!("Read history: {e}"))?;
        let has_more = entries.len() > limit as usize;
        entries.truncate(limit as usize);
        Ok(HistoryList { entries, has_more })
    }

    pub fn delete(&self, id: i64) -> Result<(), String> {
        self.connection
            .execute("DELETE FROM takes WHERE id = ?1", [id])
            .map(drop)
            .map_err(|e| format!("Delete dictation: {e}"))
    }

    /// Remove takes older than the retention window. Returns how many went.
    pub fn prune(&self, retention: HistoryRetention, now_ms: i64) -> Result<usize, String> {
        let Some(days) = retention.days() else {
            return Ok(0);
        };
        let cutoff = now_ms - days * 24 * 60 * 60 * 1000;
        self.connection
            .execute("DELETE FROM takes WHERE created_at_ms < ?1", [cutoff])
            .map_err(|e| format!("Prune history: {e}"))
    }
}

fn migrate(connection: &mut Connection) -> Result<(), OpenError> {
    let failed = |e: rusqlite::Error| OpenError::Failed(format!("Upgrade history: {e}"));
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(failed)?;
    let supported = MIGRATIONS.len() as i64;
    if version > supported {
        return Err(OpenError::NewerSchema {
            found: version,
            supported,
        });
    }
    for (sql, next_version) in MIGRATIONS.iter().zip(1_i64..).skip(version as usize) {
        let transaction = connection.transaction().map_err(failed)?;
        transaction.execute_batch(sql).map_err(failed)?;
        transaction
            .pragma_update(None, "user_version", next_version)
            .map_err(failed)?;
        transaction.commit().map_err(failed)?;
    }
    Ok(())
}

fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 24 * 60 * 60 * 1000;

    fn take(text: &str, created_at_ms: i64) -> NewTake {
        NewTake {
            text: text.into(),
            created_at_ms,
            app: Some("Notes".into()),
            model: "ggml-base.en".into(),
            audio_ms: 2_400,
            inference_ms: Some(310),
            total_ms: 720,
            status: TakeStatus::Pasted,
            detail: None,
        }
    }

    #[test]
    fn takes_round_trip_newest_first_and_survive_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/history.sqlite3");
        let skipped = NewTake {
            text: String::new(),
            app: None,
            inference_ms: None,
            status: TakeStatus::Skipped,
            detail: Some("0.12 s, below minimum 0.30 s".into()),
            ..take("", 2_000)
        };
        {
            let store = Store::open(&path).unwrap();
            store.insert(&take("first", 1_000)).unwrap();
            store.insert(&skipped).unwrap();
        }
        let entries = Store::open(&path).unwrap().list("", 500).unwrap().entries;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].status, TakeStatus::Skipped);
        assert_eq!(entries[0].app, None);
        assert_eq!(entries[0].inference_ms, None);
        assert_eq!(entries[0].detail, skipped.detail);
        assert_eq!(
            entries[1],
            HistoryEntry {
                id: entries[1].id,
                text: "first".into(),
                created_at_ms: 1_000,
                app: Some("Notes".into()),
                model: "ggml-base.en".into(),
                audio_ms: 2_400,
                inference_ms: Some(310),
                total_ms: 720,
                status: TakeStatus::Pasted,
                detail: None,
            }
        );
    }

    #[test]
    fn search_matches_text_and_app_and_treats_wildcards_literally() {
        let store = Store::in_memory();
        store.insert(&take("Pick up Coffee", 1)).unwrap();
        store
            .insert(&NewTake {
                app: Some("Slack".into()),
                ..take("ship it at 100% today", 2)
            })
            .unwrap();
        store.insert(&take("snake_case names", 3)).unwrap();
        let texts = |query: &str| {
            store
                .list(query, 500)
                .unwrap()
                .entries
                .into_iter()
                .map(|entry| entry.text)
                .collect::<Vec<_>>()
        };
        assert_eq!(texts("coffee"), ["Pick up Coffee"]);
        assert_eq!(texts("  slack "), ["ship it at 100% today"]);
        assert_eq!(texts("100%"), ["ship it at 100% today"]);
        assert_eq!(texts("%"), ["ship it at 100% today"]);
        assert_eq!(texts("_"), ["snake_case names"]);
        assert!(texts("nothing").is_empty());
        assert_eq!(texts("").len(), 3);
    }

    #[test]
    fn delete_removes_one_take() {
        let store = Store::in_memory();
        let keep = store.insert(&take("keep", 1)).unwrap();
        let gone = store.insert(&take("gone", 2)).unwrap();
        store.delete(gone).unwrap();
        store.delete(gone).unwrap();
        let ids: Vec<_> = store
            .list("", 500)
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, [keep]);
    }

    #[test]
    fn pruning_keeps_takes_inside_the_retention_window() {
        let now = 100 * DAY_MS;
        let ages = [0, DAY_MS - 1, DAY_MS + 1, 7 * DAY_MS + 1, 31 * DAY_MS];
        let cases = [
            (HistoryRetention::Day, 2),
            (HistoryRetention::Week, 3),
            (HistoryRetention::Month, 4),
            (HistoryRetention::Forever, 5),
        ];
        for (retention, kept) in cases {
            let store = Store::in_memory();
            for age in ages {
                store.insert(&take("take", now - age)).unwrap();
            }
            assert_eq!(store.prune(retention, now).unwrap(), ages.len() - kept);
            assert_eq!(
                store.list("", 500).unwrap().entries.len(),
                kept,
                "{retention:?}"
            );
        }
    }

    #[test]
    fn migration_is_idempotent_and_refuses_a_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.sqlite3");
        Store::open(&path)
            .unwrap()
            .insert(&take("kept", 1))
            .unwrap();
        let store = Store::open(&path).unwrap();
        assert_eq!(store.list("", 500).unwrap().entries.len(), 1);
        let version: i64 = store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        store
            .connection
            .pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1)
            .unwrap();
        drop(store);
        let error = Store::open(&path).err().unwrap();
        assert_eq!(
            error,
            OpenError::NewerSchema {
                found: MIGRATIONS.len() as i64 + 1,
                supported: MIGRATIONS.len() as i64
            }
        );
        let problem = error.problem();
        assert!(problem.fix.contains("Update dictate"));
        assert!(!problem.fix.contains("Restart"));
    }

    #[test]
    fn list_pages_and_reports_older_takes() {
        let store = Store::in_memory();
        for at in 1..=5 {
            store.insert(&take(&format!("take {at}"), at)).unwrap();
        }
        let page = store.list("", 2).unwrap();
        let texts: Vec<_> = page.entries.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, ["take 5", "take 4"]);
        assert!(page.has_more);
        let all = store.list("", 5).unwrap();
        assert_eq!(all.entries.len(), 5);
        assert!(!all.has_more);
        assert!(!store.list("take 1", 1).unwrap().has_more);
    }

    #[test]
    fn deleted_text_is_not_left_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.sqlite3");
        let secret = "zebra-quartz-secret-0451";
        {
            let store = Store::open(&path).unwrap();
            store.insert(&take("kept", 1)).unwrap();
            let id = store.insert(&take(secret, 2)).unwrap();
            store.delete(id).unwrap();
        }
        let mut bytes = Vec::new();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            bytes.extend(std::fs::read(entry.unwrap().path()).unwrap());
        }
        assert!(
            !bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes())
        );
        assert!(bytes.windows(4).any(|window| window == b"kept"));
    }

    #[test]
    fn typescript_contract_matches_rust_history() {
        let contract = format!(
            "export {}\nexport {}\nexport {}\n",
            TakeStatus::decl(),
            HistoryEntry::decl(),
            HistoryList::decl()
        );
        let checked_in = include_str!("../../src/history.generated.ts");
        // Prettier drops the last `;` of a one-line object type.
        let compact = |text: &str| {
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .replace(',', ";")
                .replace(";}", "}")
        };
        assert_eq!(compact(checked_in), compact(&contract));
    }

    #[test]
    fn unknown_status_is_an_error_not_a_guess() {
        let store = Store::in_memory();
        store
            .connection
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON;
                 INSERT INTO takes (text, created_at_ms, model, audio_ms, total_ms, status)
                 VALUES ('x', 1, 'm', 1, 1, 'mystery');",
            )
            .unwrap();
        assert!(store.list("", 500).unwrap_err().contains("mystery"));
    }
}
