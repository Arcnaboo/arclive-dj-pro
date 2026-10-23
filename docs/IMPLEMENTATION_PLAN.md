# Implementation Plan

## Repository state

Greenfield: only `prompt.md` existed. No Tauri/React migration.

## Stack

- Language: Rust stable (nightly accepted if host toolchain is nightly; prefer stable for release).
- UI: Slint ~1.18 + `unstable-winit-030`.
- Audio: cpal, symphonia, rtrb, realfft, rusqlite bundled.
- Stretch: WSOLA in Rust.

## Phases

1. Native shell (`ArcLive.exe`)
2. Explorer + library drag-and-drop
3. Playback (decode, play/pause, seek, meters)
4. Mixer DSP (EQ, filter, faders, crossfader, cue)
5. BPM + constant warp + sync
6. Elastic warp markers
7. SQLite library, playlists, hot cues, loops, shortcuts
8. Perf, packaging, dependency audit

## Gates

- Drop spike must prove Explorer paths before large Slint investment.
- No phase marked done without real behavior (no fake DSP/BPM).
- Build after significant changes; fix compile errors before continuing.