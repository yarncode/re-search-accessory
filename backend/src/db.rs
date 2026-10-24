use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::Serialize;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS bom_docs (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  saved_at INTEGER NOT NULL,
  payload TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS bom_draft (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  payload TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS cache (
  key TEXT PRIMARY KEY,
  body TEXT NOT NULL,
  expires_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS image_cache (
  id TEXT NOT NULL,
  size TEXT NOT NULL,
  content_type TEXT NOT NULL,
  body BLOB NOT NULL,
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (id, size)
);
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
"#;

const IMAGE_TTL_MS: i64 = 90 * 24 * 60 * 60 * 1000;
const IMAGE_LIMIT: i64 = 3000;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// `%LOCALAPPDATA%\re-search-accessory` on Windows, with a fallback for other
/// platforms and for a portable run (RES_DATA_DIR).
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RES_DATA_DIR") {
        return PathBuf::from(dir);
    }

    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local).join("re-search-accessory");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/re-search-accessory");
    }

    PathBuf::from(".")
}

/// The JLCPCB parts cache maintained by Konnect (read-only for us).
pub fn jlcpcb_path() -> PathBuf {
    if let Ok(custom) = std::env::var("RES_JLCPCB_DB") {
        return PathBuf::from(custom);
    }

    if let Ok(roaming) = std::env::var("APPDATA") {
        return PathBuf::from(roaming).join("konnect").join("jlcpcb.db");
    }

    PathBuf::from("jlcpcb.db")
}

#[derive(Debug, Serialize)]
pub struct BomMeta {
    pub id: String,
    pub name: String,
    pub saved_at: i64,
}

#[derive(Debug, Serialize)]
pub struct BomRecord {
    pub id: String,
    pub name: String,
    pub saved_at: i64,
    pub payload: String,
}

#[derive(Debug, Serialize)]
pub struct Draft {
    pub payload: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct JlcPart {
    pub lcsc: String,
    pub mpn: String,
    pub package: String,
    pub manufacturer: String,
    pub description: String,
    pub datasheet: String,
    pub price: f64,
    pub stock: i64,
    pub category: String,
}

#[derive(Debug, Serialize)]
pub struct JlcStatus {
    pub available: bool,
    pub path: String,
    pub size: u64,
    pub parts: i64,
    pub error: Option<String>,
}

pub struct Db {
    conn: Mutex<Connection>,
    memory_only: bool,
}

impl Db {
    pub fn open() -> Result<Self, String> {
        let dir = data_dir();

        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("cannot create data dir {}: {err}", dir.display()))?;

        let path = dir.join("app.db");
        let conn = Connection::open(&path).map_err(|err| format!("cannot open {}: {err}", path.display()))?;

        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|err| format!("cannot enable WAL: {err}"))?;
        conn.execute_batch(SCHEMA)
            .map_err(|err| format!("cannot create schema: {err}"))?;

        let db = Self {
            conn: Mutex::new(conn),
            memory_only: false,
        };
        db.prune();
        tracing::info!(path = %path.display(), "sqlite store ready");
        Ok(db)
    }

    /// Fallback when the data directory is not writable.
    pub fn in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|err| format!("cannot open memory db: {err}"))?;

        conn.execute_batch(SCHEMA)
            .map_err(|err| format!("cannot create schema: {err}"))?;

        Ok(Self {
            conn: Mutex::new(conn),
            memory_only: true,
        })
    }

    pub fn is_memory_only(&self) -> bool {
        self.memory_only
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|_| "sqlite mutex poisoned".to_string())
    }

    fn prune(&self) {
        let Ok(conn) = self.conn() else {
            return;
        };
        let now = now_ms();
        let _ = conn.execute("DELETE FROM cache WHERE expires_at <= ?1", params![now]);
        let _ = conn.execute(
            "DELETE FROM image_cache WHERE fetched_at < ?1",
            params![now - IMAGE_TTL_MS],
        );
        let _ = conn.execute(
            "DELETE FROM image_cache WHERE rowid IN (
                SELECT rowid FROM image_cache ORDER BY fetched_at ASC
                LIMIT MAX(0, (SELECT COUNT(*) FROM image_cache) - ?1)
            )",
            params![IMAGE_LIMIT],
        );
    }

    /* ---------------- BOM documents ---------------- */
    pub fn list_boms(&self) -> Result<Vec<BomMeta>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT id, name, saved_at FROM bom_docs ORDER BY saved_at DESC")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BomMeta {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    saved_at: row.get(2)?,
                })
            })
            .map_err(|err| err.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
    }

    pub fn get_bom(&self, id: &str) -> Result<Option<BomRecord>, String> {
        let conn = self.conn()?;

        conn.query_row(
            "SELECT id, name, saved_at, payload FROM bom_docs WHERE id = ?1",
            params![id],
            |row| {
                Ok(BomRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    saved_at: row.get(2)?,
                    payload: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())
    }

    pub fn save_bom(&self, id: &str, name: &str, payload: &str) -> Result<i64, String> {
        let conn = self.conn()?;
        let now = now_ms();

        conn.execute(
            "INSERT INTO bom_docs (id, name, saved_at, payload) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, saved_at = excluded.saved_at, payload = excluded.payload",
            params![id, name, now, payload],
        )
        .map_err(|err| err.to_string())?;

        Ok(now)
    }

    pub fn delete_bom(&self, id: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM bom_docs WHERE id = ?1", params![id])
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn get_draft(&self) -> Result<Option<Draft>, String> {
        let conn = self.conn()?;

        conn.query_row(
            "SELECT payload, updated_at FROM bom_draft WHERE id = 1",
            [],
            |row| {
                Ok(Draft {
                    payload: row.get(0)?,
                    updated_at: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(|err| err.to_string())
    }

    pub fn save_draft(&self, payload: &str) -> Result<i64, String> {
        let conn = self.conn()?;
        let now = now_ms();

        conn.execute(
            "INSERT INTO bom_draft (id, payload, updated_at) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated_at = excluded.updated_at",
            params![payload, now],
        )
        .map_err(|err| err.to_string())?;

        Ok(now)
    }

    /* ---------------- settings ---------------- */
    /// Small key/value store for UI preferences that must survive a restart
    /// (theme, last KiCad project, last gerber folder).
    pub fn settings(&self) -> Result<std::collections::HashMap<String, serde_json::Value>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings ORDER BY key")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|err| err.to_string())?;

        let mut map = std::collections::HashMap::new();

        for row in rows {
            let (key, value) = row.map_err(|err| err.to_string())?;
            let parsed = serde_json::from_str(&value).unwrap_or(serde_json::Value::Null);
            map.insert(key, parsed);
        }

        Ok(map)
    }

    pub fn set_setting(&self, key: &str, value: &serde_json::Value) -> Result<(), String> {
        let conn = self.conn()?;
        let text = serde_json::to_string(value).map_err(|err| err.to_string())?;

        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, text, now_ms()],
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    /* ---------------- offline cache ---------------- */
    pub fn cache_get(&self, key: &str) -> Option<String> {
        let conn = self.conn().ok()?;

        conn.query_row(
            "SELECT body FROM cache WHERE key = ?1 AND expires_at > ?2",
            params![key, now_ms()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
    }

    pub fn cache_put(&self, key: &str, body: &str, ttl: Duration) {
        let Ok(conn) = self.conn() else {
            return;
        };
        let expires = now_ms() + ttl.as_millis() as i64;

        let _ = conn.execute(
            "INSERT INTO cache (key, body, expires_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET body = excluded.body, expires_at = excluded.expires_at",
            params![key, body, expires],
        );
    }

    pub fn image_get(&self, id: &str, size: &str) -> Option<(String, Vec<u8>)> {
        let conn = self.conn().ok()?;

        conn.query_row(
            "SELECT content_type, body FROM image_cache WHERE id = ?1 AND size = ?2",
            params![id, size],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .optional()
        .ok()
        .flatten()
    }

    pub fn image_put(&self, id: &str, size: &str, content_type: &str, body: &[u8]) {
        let Ok(conn) = self.conn() else {
            return;
        };

        let _ = conn.execute(
            "INSERT INTO image_cache (id, size, content_type, body, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id, size) DO UPDATE SET content_type = excluded.content_type, body = excluded.body, fetched_at = excluded.fetched_at",
            params![id, size, content_type, body, now_ms()],
        );
    }

    /* ---------------- JLCPCB read-only cache ---------------- */
    pub fn jlc_status(&self) -> JlcStatus {
        let path = jlcpcb_path();
        let display = path.display().to_string();

        if !path.is_file() {
            return JlcStatus {
                available: false,
                path: display,
                size: 0,
                parts: 0,
                error: Some("Chưa có jlcpcb.db (Konnect tạo file này khi tải database linh kiện).".to_string()),
            };
        }

        let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);

        match open_jlc(&path).and_then(|conn| {
            conn.query_row("SELECT COUNT(*) FROM components", [], |row| row.get::<_, i64>(0))
                .map_err(|err| err.to_string())
        }) {
            Ok(parts) => JlcStatus {
                available: true,
                path: display,
                size,
                parts,
                error: None,
            },
            Err(err) => JlcStatus {
                available: false,
                path: display,
                size,
                parts: 0,
                error: Some(err),
            },
        }
    }

    pub fn jlc_lookup(&self, mpn: &str, limit: usize) -> Result<Vec<JlcPart>, String> {
        let path = jlcpcb_path();

        if !path.is_file() {
            return Err("Chưa có jlcpcb.db".to_string());
        }

        let conn = open_jlc(&path)?;
        let limit = limit.clamp(1, 20) as i64;
        let needle = mpn.trim();

        if needle.is_empty() {
            return Ok(Vec::new());
        }

        let mut parts: Vec<JlcPart> = Vec::new();
        let mut seen: Vec<String> = Vec::new();

        let sql_exact = "SELECT LCSC, MFR_Part, Package, Manufacturer, Description, Datasheet, Price, Stock, Category
                         FROM components WHERE MFR_Part = ?1 COLLATE NOCASE LIMIT ?2";
        let sql_prefix = "SELECT LCSC, MFR_Part, Package, Manufacturer, Description, Datasheet, Price, Stock, Category
                          FROM components WHERE MFR_Part LIKE ?1 || '%' COLLATE NOCASE LIMIT ?2";
        let sql_lcsc = "SELECT LCSC, MFR_Part, Package, Manufacturer, Description, Datasheet, Price, Stock, Category
                        FROM components WHERE LCSC = ?1 COLLATE NOCASE LIMIT ?2";

        for (sql, param) in [(sql_exact, needle), (sql_prefix, needle), (sql_lcsc, needle)] {
            let Ok(mut stmt) = conn.prepare(sql) else {
                continue;
            };
            let Ok(rows) = stmt.query_map(params![param, limit], map_jlc) else {
                continue;
            };

            for row in rows.flatten() {
                if seen.iter().any(|lcsc| lcsc == &row.lcsc) {
                    continue;
                }
                seen.push(row.lcsc.clone());
                parts.push(row);

                if parts.len() >= limit as usize {
                    return Ok(parts);
                }
            }
        }

        Ok(parts)
    }
}

fn open_jlc(path: &PathBuf) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| format!("cannot open {}: {err}", path.display()))?;

    conn.busy_timeout(Duration::from_secs(2))
        .map_err(|err| err.to_string())?;

    Ok(conn)
}

fn map_jlc(row: &rusqlite::Row<'_>) -> rusqlite::Result<JlcPart> {
    Ok(JlcPart {
        lcsc: row.get(0)?,
        mpn: row.get(1)?,
        package: row.get(2)?,
        manufacturer: row.get(3)?,
        description: row.get(4)?,
        datasheet: row.get(5)?,
        price: row.get(6)?,
        stock: row.get(7)?,
        category: row.get(8)?,
    })
}
