//! Windows Explorer file drop hit-testing.
//!
//! Slint DropArea is in-app only. Explorer paths arrive via OLE
//! `IDropTarget` or winit `HoveredFile` / `DroppedFile`. Destination is
//! chosen from cursor position against published deck/library rectangles
//! in **logical** pixels.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropZone {
    DeckA,
    DeckB,
    Library,
    #[default]
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

    /// Hit-test a physical (device) point using the window scale factor.
    pub fn hit_test_physical(&self, px: f64, py: f64, scale: f64) -> DropZone {
        let scale = scale.max(0.1);
        self.hit_test(px / scale, py / scale)
    }
}

/// Convert physical client coordinates to logical UI coordinates.
pub fn physical_to_logical(px: f64, py: f64, scale: f64) -> (f64, f64) {
    let scale = scale.max(0.1);
    (px / scale, py / scale)
}

/// Coalesce multiple winit `DroppedFile` events into one decision.
#[derive(Debug, Default)]
pub struct DropAccumulator {
    paths: Vec<PathBuf>,
    first_at: Option<Instant>,
    zone: DropZone,
    cursor: (f64, f64),
}

impl DropAccumulator {
    pub fn push(&mut self, path: PathBuf, zone: DropZone, cursor: (f64, f64)) {
        if self.paths.is_empty() {
            self.first_at = Some(Instant::now());
            self.zone = zone;
            self.cursor = cursor;
        }
        if !self.paths.iter().any(|p| p == &path) {
            self.paths.push(path);
        }
        // Prefer a real zone if a later event hits a target.
        if zone != DropZone::None {
            self.zone = zone;
            self.cursor = cursor;
        }
    }

    pub fn ready(&self, window: Duration) -> bool {
        match self.first_at {
            Some(t) => t.elapsed() >= window,
            None => false,
        }
    }

    pub fn take(&mut self) -> Option<(DropZone, Vec<PathBuf>, (f64, f64))> {
        if self.paths.is_empty() {
            return None;
        }
        let paths = std::mem::take(&mut self.paths);
        let zone = self.zone;
        let cursor = self.cursor;
        self.first_at = None;
        self.zone = DropZone::None;
        Some((zone, paths, cursor))
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
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

    #[test]
    fn dpi_hit_test_maps_physical_to_logical() {
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
        // 150% DPI: logical (10,10) is physical (15,15)
        assert_eq!(layout.hit_test_physical(15.0, 15.0, 1.5), DropZone::DeckA);
        assert_eq!(layout.hit_test_physical(225.0, 15.0, 1.5), DropZone::DeckB);
        let (lx, ly) = physical_to_logical(30.0, 30.0, 2.0);
        assert!((lx - 15.0).abs() < 1e-9 && (ly - 15.0).abs() < 1e-9);
    }

    #[test]
    fn multi_file_library_imports() {
        let paths = vec![PathBuf::from("a.wav"), PathBuf::from("b.mp3")];
        match decide_drop(DropZone::Library, &paths, false, true) {
            DropDecision::ImportLibrary { paths: p } => assert_eq!(p.len(), 2),
            other => panic!("expected import, got {other:?}"),
        }
    }

    #[test]
    fn accumulator_coalesces_paths() {
        let mut acc = DropAccumulator::default();
        acc.push(PathBuf::from("a.wav"), DropZone::DeckA, (1.0, 2.0));
        acc.push(PathBuf::from("b.wav"), DropZone::DeckA, (1.0, 2.0));
        let (zone, paths, _) = acc.take().unwrap();
        assert_eq!(zone, DropZone::DeckA);
        assert_eq!(paths.len(), 2);
    }
}