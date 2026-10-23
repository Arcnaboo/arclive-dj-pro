use crate::drop_target::is_supported_audio;
use std::path::{Path, PathBuf};

pub fn scan_folder(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let walker = walkdir_simple(root, 8);
    for p in walker {
        if is_supported_audio(&p) {
            out.push(p);
        }
    }
    out
}

fn walkdir_simple(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < max_depth {
                    stack.push((path, depth + 1));
                }
            } else {
                out.push(path);
            }
        }
    }
    out
}