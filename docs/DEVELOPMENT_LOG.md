# Development Log

## 2026-10-23 — Project start

- Repository contained only `prompt.md`.
- Chose Slint + winit file-drop adapter for Explorer DnD.
- Chose WASAPI/cpal; ASIO deferred for licensing.
- Chose Rust WSOLA for default pitch-preserving stretch; Rubber Band optional later.
- Slint royalty-free desktop license with attribution in About/Settings.

## Verification notes

- [x] Explorer `DroppedFile` / `HoveredFile` hooked via `unstable-winit-030` + `on_winit_window_event`
- [x] Hit-test selects Deck A / B / Library from cursor (+ Win32 `GetCursorPos`/`ScreenToClient`)
- [x] `cargo build --release` → `target/release/ArcLive.exe` (~17 MB)
- [x] Unit tests: 14 passed (drop, EQ, crossfader, WSOLA, BPM, warp map, sync phase)
- [x] App launches (`MainWindowTitle = ArcLive DJ`), native Slint UI (no webview)
- [x] WAV load (`assets/test_138.wav`) → Deck A metadata, waveform, BPM ~139.67, warp target 146 BPM, library row Ready
- [x] Decode pipeline: symphonia (WAV/AIFF/FLAC/MP3 via `all` features)
- [x] Real EQ / filter / faders / crossfader curves in audio callback
- [x] WSOLA constant-tempo warp + phase-sync helper
- [x] SQLite library + analysis cache + playlists/hot cues tables
- [ ] Manual Explorer drag verification on a clean Windows profile (hook installed; confirm with Explorer)
- [ ] Full dual-output cue stream (limitation shown when only one device)

## Dependency / packaging notes

- PE binary is a standard Windows EXE (PE32+).
- Runtime uses system WASAPI and the Slint/winit graphics stack (typically OpenGL/ANGLE or similar via OS).
- `rusqlite` uses the `bundled` feature — no separate `sqlite3.dll`.
- Do **not** claim single-file absolute portability until validated on a clean VM; redistribute any discovered companion DLLs if present.

## Build

```powershell
cargo build --release
# → target\release\ArcLive.exe
```

Optional CLI:

```powershell
.\target\release\ArcLive.exe --load-a .\assets\test_138.wav
.\target\release\ArcLive.exe --load-b .\assets\test_146.wav
```

## Keyboard shortcuts

| Key | Action |
|---|---|
| Space | Play/Pause Deck A |
| Q | Cue Deck A |
| W | Play/Pause Deck B |
| E | Cue Deck B |
| S | Sync Deck B |
| , | Settings |
