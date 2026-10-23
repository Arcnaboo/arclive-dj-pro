//! Windows Explorer file drop hit-testing.
//!
//! Slint DropArea is in-app only. Explorer paths arrive via winit
//! `HoveredFile` / `DroppedFile`. Destination is chosen from cursor
//! position against published deck/library rectangles.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    DeckA,
    DeckB,
    Library,
    None,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }

    pub fn is_valid(&self) -> bool {
        self.w > 1.0 && self.h > 1.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct DropLayout {
    pub deck_a: Rect,
    pub deck_b: Rect,
    pub library: Rect,
}

impl DropLayout {
    pub fn hit_test(&self, x: f64, y: f64) -> DropZone {
        // Prefer decks over library when overlapping (should not happen).
        if self.deck_a.is_valid() && self.deck_a.contains(x, y) {
            return DropZone::DeckA;
        }
        if self.deck_b.is_valid() && self.deck_b.contains(x, y) {
            return DropZone::DeckB;
        }
        if self.library.is_valid() && self.library.contains(x, y) {
            return DropZone::Library;
        }
        DropZone::None
    }
}

pub const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "aiff", "aif", "flac", "mp3"];

pub fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.iter().any(|s| e.eq_ignore_ascii_case(s)))
        .unwrap_or(false)
}

pub fn format_supported_list() -> &'static str {
    "WAV, AIFF, FLAC, MP3"
}

#[derive(Debug, Clone)]
pub enum DropDecision {
    LoadDeck { zone: DropZone, path: PathBuf },
    ImportLibrary { paths: Vec<PathBuf> },
    Reject { message: String },
}

pub fn decide_drop(
    zone: DropZone,
    paths: &[PathBuf],
    deck_playing: bool,
    load_protection: bool,
) -> DropDecision {
    match zone {
        DropZone::None => DropDecision::Reject {
            message: "Drop onto Deck A, Deck B, or the library.".into(),
        },
        DropZone::Library => {
            let valid: Vec<PathBuf> = paths
                .iter()
                .filter(|p| is_supported_audio(p))
                .cloned()
                .collect();
            if valid.is_empty() {
                DropDecision::Reject {
                    message: format!(
                        "Unsupported file. Supported formats: {}.",
                        format_supported_list()
                    ),
                }
            } else {
                DropDecision::ImportLibrary { paths: valid }
            }
        }
        DropZone::DeckA | DropZone::DeckB => {
            if paths.len() > 1 {
                return DropDecision::Reject {
                    message: "Drop a single file onto a deck (or multiple files onto the library)."
                        .into(),
                };
            }
            let Some(path) = paths.first() else {
                return DropDecision::Reject {
                    message: "No file path in drop.".into(),
                };
            };
            if !is_supported_audio(path) {
                return DropDecision::Reject {
                    message: format!(
                        "Unsupported file. Supported formats: {}.",
                        format_supported_list()
                    ),
                };
            }
            if deck_playing && load_protection {
                return DropDecision::Reject {
                    message: "Load protection: stop the deck or disable protection in Settings."
                        .into(),
                };
            }
            DropDecision::LoadDeck {
                zone,
                path: path.clone(),
            }
        }
    }
}

#[cfg(windows)]
pub fn cursor_client_pos(hwnd: isize) -> Option<(f64, f64)> {
    use std::mem::MaybeUninit;

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    extern "system" {
        fn GetCursorPos(lp_point: *mut POINT) -> i32;
        fn ScreenToClient(h_wnd: isize, lp_point: *mut POINT) -> i32;
    }

    unsafe {
        let mut pt = MaybeUninit::<POINT>::uninit();
        if GetCursorPos(pt.as_mut_ptr()) == 0 {
            return None;
        }
        let mut pt = pt.assume_init();
        if ScreenToClient(hwnd, &mut pt) == 0 {
            return None;
        }
        Some((pt.x as f64, pt.y as f64))
    }
}

#[cfg(not(windows))]
pub fn cursor_client_pos(_hwnd: isize) -> Option<(f64, f64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_decks() {
        let layout = DropLayout {
            deck_a: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            deck_b: Rect {
                x: 120.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            library: Rect {
                x: 0.0,
                y: 120.0,
                w: 220.0,
                h: 80.0,
            },
        };
        assert_eq!(layout.hit_test(10.0, 10.0), DropZone::DeckA);
        assert_eq!(layout.hit_test(150.0, 10.0), DropZone::DeckB);
        assert_eq!(layout.hit_test(10.0, 150.0), DropZone::Library);
        assert_eq!(layout.hit_test(500.0, 500.0), DropZone::None);
    }

    #[test]
    fn multi_file_deck_rejected() {
        let paths = vec![PathBuf::from("a.wav"), PathBuf::from("b.wav")];
        match decide_drop(DropZone::DeckA, &paths, false, true) {
            DropDecision::Reject { .. } => {}
            other => panic!("expected reject, got {other:?}"),
        }
    }

    #[test]
    fn supported_extensions() {
        assert!(is_supported_audio(Path::new("track.MP3")));
        assert!(is_supported_audio(Path::new("x.flac")));
        assert!(!is_supported_audio(Path::new("doc.pdf")));
    }
}