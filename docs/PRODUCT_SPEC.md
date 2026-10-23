# ArcLive DJ — Product Specification

## Vision

ArcLive is a two-deck native Windows DJ performance workstation. Concept: Ableton Live automatic warping + Traktor mixing workflow + Apple interface refinement + Rust performance. Optimized for long electronic tracks (psytrance, progressive trance, techno).

## Non-negotiables

- Windows executable `ArcLive.exe` (no browser, WebView, Electron, Tauri, React, JS, HTML, CSS).
- Native Rust UI (Slint).
- Real Windows Explorer drag-and-drop.
- Offline operation.
- Real audio DSP (no simulated EQ, fake playback, or fabricated BPM).

## Core user flow

1. Launch ArcLive.
2. Set master BPM (e.g. 146.00).
3. Drag a track onto Deck A → analyze → prepare for master BPM.
4. Play.
5. Drag a track onto Deck B → analyze → warp → headphone cue → Sync.
6. Mix with EQ, channel faders, and crossfader.

Tempo matching and beat-phase synchronization are separate behaviors. Sync engages phase alignment to the global musical clock.

## Acceptance criteria

See prompt §25. Summary:

1. `ArcLive.exe` launches on Windows without browser/webview/server.
2. Explorer WAV → Deck A and MP3 → Deck B land on the correct deck.
3. Metadata, waveforms, BPM analysis, pitch-preserving tempo match, sync, EQ, faders, crossfader, cue routing (when dual outputs exist), and library search all work with real audio.
4. Playback remains stable during background analysis.
5. Packaging and launch are documented with actual DLL/runtime requirements.

## Out of scope (initial)

- Full DAW editing, multi-track arrangement, cloud sync, plugin hosting.
- ASIO in the default redistributable build (licensing).