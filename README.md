# ArcLive DJ

Native Rust two-deck DJ workstation for Windows.

**100% native** — Slint UI, no Electron/Tauri/WebView. Real WASAPI audio, BPM analysis, pitch-preserving WSOLA warp, Explorer drag-and-drop.

## Build

```powershell
cargo build --release
```

Executable: `target\release\ArcLive.exe`

## Run

```powershell
cargo run --release
```

## Requirements

- Windows 10/11
- Rust toolchain
- WASAPI output device

See `docs/` for architecture, drag-and-drop, warp engine, and testing notes.

## License notes

- Application: MIT (see project intent)
- Slint: royalty-free desktop use with attribution (About panel)
- Rubber Band is **not** linked by default (optional later; GPL or commercial)
