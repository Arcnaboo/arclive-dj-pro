# Drag-and-Drop Specification

## Requirement

First-class Windows Explorer file drops onto Deck A, Deck B, and the library. Destination is determined by pointer position. Do not guess.

## Mechanism

Slint `DropArea` handles **in-app** library → deck drags (opaque track id).

**Explorer** drops use winit via Slint `unstable-winit-030`:

- `HoveredFile` / `HoveredFileCancelled` / `DroppedFile`
- Cursor client position from `CursorMoved` or Win32 `GetCursorPos` + `ScreenToClient`
- Hit-test against rectangles published by the UI (Deck A, Deck B, Library)

Fallback: Windows `IDropTarget` on HWND. Framework fallback: egui/eframe.

## Policies

| Case | Behavior |
|---|---|
| One audio file → Deck A/B | Load that deck |
| Multi-file → Library | Import all valid |
| Multi-file → Deck | Reject; explain; leave playing deck alone |
| Unsupported type | Message; supported: WAV, AIFF, FLAC, MP3 |
| Playing deck + load protection | Block / confirm; default = block |
| Drop outside targets | No-op |

## Hover feedback

- Deck A: blue border, brighten, “Drop to load Deck A”
- Deck B: amber border, “Drop to load Deck B”
- Only one target highlighted

## Acceptance tests

WAV→A, MP3→B, FLAC→library, multi→library, unsupported, outside, active deck, library→A/B, long paths, Unicode, spaces in path. Complete only after real Explorer verification.