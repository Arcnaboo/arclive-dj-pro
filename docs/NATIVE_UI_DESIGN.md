# Native UI Design

## Framework choice: Slint

Selected for custom controls, hardware-accelerated rendering, High-DPI support, and a designed (non-OS-widget) aesthetic. Declarative `.slint` is permitted as part of the Rust-native stack.

**License:** Royalty-free desktop license with Slint attribution in About/Settings. Documented in DEVELOPMENT_LOG.

**Fallback:** egui/eframe only if Explorer file drops cannot be made reliable via winit `on_winit_window_event` or a Windows `IDropTarget` adapter.

## Theme tokens

| Token | Hex |
|---|---|
| Background | `#111316` |
| Main panel | `#1B1E23` |
| Elevated | `#24282E` |
| Borders | `#343941` |
| Primary text | `#F2F3F5` |
| Secondary text | `#989FA9` |
| Deck A | `#62B9E8` |
| Deck B | `#E6AD70` |
| Synced | `#71C9A2` |
| Warning | `#E5B96D` |
| Error | `#E87D7D` |

Typography: Segoe UI, tabular numerals for BPM/time. Spacing: 4 px grid. Default window 1920×1080; persist size in `%APPDATA%\ArcLive\config.json`.

## Layout

Top toolbar (logo, master BPM, audio, settings) → dual decks with waveforms and transport → mixer strip → collapsible library table.

## Interaction principles

Restrained dark UI. No neon cyberpunk, fake metal knobs, or oversized glass. Knobs: vertical drag, Shift fine, double-click reset. Drop targets: subtle dashed empty state; hover highlight follows pointer.