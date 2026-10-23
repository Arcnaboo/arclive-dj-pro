use crate::audio::decoder::TrackMeta;
use crate::library::database::TrackRow;
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub fn track_row_from_meta(meta: &TrackMeta, status: &str) -> TrackRow {
    let path = Path::new(&meta.path);
    let (size, mtime) = file_stamp(path);
    TrackRow {
        id: Uuid::new_v4().to_string(),
        path: meta.path.clone(),
        title: meta.title.clone(),
        artist: if meta.artist.is_empty() {
            "Unknown Artist".into()
        } else {
            meta.artist.clone()
        },
        bpm: 0.0,
        duration_secs: meta.duration_secs,
        key: String::new(),
        format: meta.format.clone(),
        status: status.into(),
        file_size: size,
        mtime,
    }
}

pub fn file_stamp(path: &Path) -> (u64, i64) {
    match fs::metadata(path) {
        Ok(m) => {
            let size = m.len();
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            (size, mtime)
        }
        Err(_) => (0, 0),
    }
}