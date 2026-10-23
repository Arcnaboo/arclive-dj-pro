use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TrackRow {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub bpm: f64,
    pub duration_secs: f64,
    pub key: String,
    pub format: String,
    pub status: String,
    pub file_size: u64,
    pub mtime: i64,
}

pub struct LibraryDb {
    conn: Connection,
}

impl LibraryDb {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).context("open library db")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY,
                path TEXT UNIQUE NOT NULL,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                bpm REAL NOT NULL DEFAULT 0,
                duration_secs REAL NOT NULL DEFAULT 0,
                key TEXT NOT NULL DEFAULT '',
                format TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'Imported',
                file_size INTEGER NOT NULL DEFAULT 0,
                mtime INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS playlists (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS playlist_tracks (
                playlist_id TEXT NOT NULL,
                track_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY (playlist_id, track_id)
            );
            CREATE TABLE IF NOT EXISTS analysis_cache (
                path TEXT PRIMARY KEY,
                file_size INTEGER NOT NULL,
                mtime INTEGER NOT NULL,
                bpm REAL NOT NULL,
                confidence REAL NOT NULL,
                offset_secs REAL NOT NULL,
                warp_json TEXT NOT NULL DEFAULT '{}'
            );
            CREATE TABLE IF NOT EXISTS hot_cues (
                track_id TEXT NOT NULL,
                slot INTEGER NOT NULL,
                position_secs REAL NOT NULL,
                label TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (track_id, slot)
            );
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_track(&self, t: &TrackRow) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO tracks (id, path, title, artist, bpm, duration_secs, key, format, status, file_size, mtime)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(path) DO UPDATE SET
                title=excluded.title,
                artist=excluded.artist,
                bpm=excluded.bpm,
                duration_secs=excluded.duration_secs,
                key=excluded.key,
                format=excluded.format,
                status=excluded.status,
                file_size=excluded.file_size,
                mtime=excluded.mtime
            "#,
            params![
                t.id,
                t.path,
                t.title,
                t.artist,
                t.bpm,
                t.duration_secs,
                t.key,
                t.format,
                t.status,
                t.file_size as i64,
                t.mtime
            ],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<Vec<TrackRow>> {
        let q = format!("%{}%", query.trim());
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, path, title, artist, bpm, duration_secs, key, format, status, file_size, mtime
            FROM tracks
            WHERE ?1 = '%%' OR title LIKE ?1 OR artist LIKE ?1 OR path LIKE ?1
            ORDER BY title COLLATE NOCASE
            "#,
        )?;
        let rows = stmt.query_map(params![q], |r| {
            Ok(TrackRow {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                bpm: r.get(4)?,
                duration_secs: r.get(5)?,
                key: r.get(6)?,
                format: r.get(7)?,
                status: r.get(8)?,
                file_size: r.get::<_, i64>(9)? as u64,
                mtime: r.get(10)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn all(&self) -> Result<Vec<TrackRow>> {
        self.search("")
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<TrackRow>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, path, title, artist, bpm, duration_secs, key, format, status, file_size, mtime
            FROM tracks WHERE id = ?1
            "#,
        )?;
        let mut rows = stmt.query_map(params![id], |r| {
            Ok(TrackRow {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                bpm: r.get(4)?,
                duration_secs: r.get(5)?,
                key: r.get(6)?,
                format: r.get(7)?,
                status: r.get(8)?,
                file_size: r.get::<_, i64>(9)? as u64,
                mtime: r.get(10)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn create_playlist(&self, name: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO playlists (id, name) VALUES (?1, ?2)",
                params![id, name],
            )?;
        Ok(id)
    }

    pub fn add_to_playlist(&self, playlist_id: &str, track_id: &str, position: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, position],
        )?;
        Ok(())
    }

    pub fn set_hot_cue(&self, track_id: &str, slot: i64, position_secs: f64, label: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO hot_cues (track_id, slot, position_secs, label) VALUES (?1, ?2, ?3, ?4)",
            params![track_id, slot, position_secs, label],
        )?;
        Ok(())
    }

    pub fn cache_analysis(
        &self,
        path: &str,
        file_size: u64,
        mtime: i64,
        bpm: f64,
        confidence: f64,
        offset_secs: f64,
        warp_json: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO analysis_cache (path, file_size, mtime, bpm, confidence, offset_secs, warp_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(path) DO UPDATE SET
                file_size=excluded.file_size,
                mtime=excluded.mtime,
                bpm=excluded.bpm,
                confidence=excluded.confidence,
                offset_secs=excluded.offset_secs,
                warp_json=excluded.warp_json
            "#,
            params![path, file_size as i64, mtime, bpm, confidence, offset_secs, warp_json],
        )?;
        Ok(())
    }

    pub fn get_cached_analysis(
        &self,
        path: &str,
        file_size: u64,
        mtime: i64,
    ) -> Result<Option<(f64, f64, f64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT bpm, confidence, offset_secs, warp_json, file_size, mtime FROM analysis_cache WHERE path = ?1",
        )?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let size: i64 = row.get(4)?;
            let mt: i64 = row.get(5)?;
            if size as u64 == file_size && mt == mtime {
                return Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)));
            }
        }
        Ok(None)
    }
}

pub fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ArcLive")
        .join("library.db")
}