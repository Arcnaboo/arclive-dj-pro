# ARCLIVE DJ — DEFINITIVE MASTER CURSOR AGENT PROMPT
## 100% Native Rust Desktop DJ Application
### Standalone Windows EXE • Native UI • Drag-and-Drop • Automatic Warping • Professional Audio Engine

---

# 0. CRITICAL ARCHITECTURE CORRECTION

The previous application specification incorrectly proposed Tauri, React, TypeScript, Vite, and a web-based UI.

**That architecture is rejected.**

ArcLive DJ must be a genuine native desktop application, developed primarily in Rust, with no embedded browser or web frontend.

### Absolute requirements

- The final application must be a Windows executable named `ArcLive.exe`.
- The desktop UI must use a native Rust-compatible rendering framework.
- All audio processing must be implemented in Rust or through carefully selected native DSP libraries called from Rust.
- Track loading must support Windows Explorer drag-and-drop.
- The application must work completely offline.
- No local web server.
- No Electron.
- No Tauri.
- No React.
- No TypeScript.
- No JavaScript.
- No HTML.
- No CSS.
- No Chromium.
- No WebView2.
- No browser runtime.
- No Python runtime.
- No Node.js runtime.

Do not build a web application packaged as a desktop application.

Do not generate a fake DJ interface.

Do not replace the native implementation with a simpler web-based solution.

The final application must be a functional Windows desktop program.

**Important:** A standalone EXE does not necessarily mean every native dependency can be statically linked. Prefer a self-contained distribution, investigate licensing and runtime requirements, and document any required DLLs or system components. Never claim single-file portability until verified on a clean Windows installation.

---

# 1. YOUR ROLE

Act as a senior engineering team consisting of:

1. Principal Rust systems architect.
2. Professional real-time audio DSP engineer.
3. Windows desktop application engineer.
4. Native UI rendering engineer.
5. Digital audio workstation architect.
6. Professional DJ workflow designer.
7. Apple-quality interaction and visual design specialist.
8. Software testing and performance engineer.

Your responsibility is to build **ArcLive DJ**, a real professional desktop DJ application.

The concept:

**Ableton Live's automatic audio warping + Traktor's DJ mixing workflow + Apple's interface refinement + Rust's native performance.**

This is a performance application, not a DAW clone.

It should be optimized for playing and mixing long electronic music tracks, particularly psychedelic trance, progressive trance, techno, and related styles.

---

# 2. TECHNOLOGY STACK

## 2.1 Primary Language

Rust, stable toolchain.

Use Rust for:

- Application lifecycle.
- Native desktop integration.
- Audio engine.
- Track loading.
- Audio decoding.
- BPM analysis.
- Beat tracking.
- Warping.
- Time stretching.
- Synchronization.
- DSP.
- Mixing.
- Audio device management.
- Music library.
- Persistence.
- Waveform processing.
- File drag-and-drop.
- MIDI support in later phases.

Avoid introducing another programming language unless a justified build-time or DSP integration requirement exists.

## 2.2 Native UI Framework

**Preferred: Slint.**

Evaluate its current capabilities for:

- Native Windows application development.
- Custom-designed controls.
- Hardware-accelerated rendering.
- High-DPI support.
- Custom waveform visualization.
- File drag-and-drop.
- Keyboard and pointer input.
- Accessibility.
- Packaging.
- Commercial licensing.

Slint is preferred because ArcLive needs a carefully designed, high-quality interface rather than generic operating-system widgets.

Slint's declarative UI language is permitted as part of the Rust-native UI framework. It is not HTML, CSS, or JavaScript.

If Slint presents a blocking technical or licensing limitation, evaluate:

- egui/eframe.
- iced.
- Another actively maintained Rust-native GUI framework.

Choose one framework and document the reasoning.

Do not mix multiple UI frameworks unnecessarily.

Do not fall back to Tauri or web technologies.

## 2.3 Audio Libraries

Evaluate:

- `cpal` for audio device access.
- `symphonia` for audio decoding.
- `rubato` for sample-rate conversion where appropriate.
- `realfft` or `rustfft` for spectral analysis.
- `rtrb` or another verified bounded ring-buffer solution for real-time communication.
- `rusqlite` for the track database.

For time stretching, evaluate Rubber Band or another suitable native DSP implementation.

Do not confuse sample-rate conversion with pitch-preserving time stretching.

Verify licensing, performance, and supported platforms.

## 2.4 Windows Integration

Support:

- Windows 11.
- Windows Explorer drag-and-drop.
- Native file dialogs.
- Native application icon.
- High-DPI monitors.
- Windows audio devices.
- WASAPI.
- ASIO where feasible and legally distributable.
- Persistent application settings.
- Graceful device disconnects.

The application must launch directly from Windows.

No browser or server should be involved.

---

# 3. FINAL DELIVERABLE

The project must produce a release executable:

`target/release/ArcLive.exe`

The user must be able to double-click the executable and launch the application.

Implement an appropriate Cargo release configuration.

Provide a reproducible build command such as:

```powershell
cargo build --release
```

Ensure the executable name is configured correctly.

If native dependencies require additional files, produce a complete distributable folder and document those requirements.

Test deployment on a clean Windows environment where possible.

Do not claim a single-file executable if the application still requires undistributed DLLs or assets.

---

# 4. PRODUCT VISION

ArcLive is a two-deck DJ performance workstation.

The user should be able to:

1. Open ArcLive.
2. Set the global BPM.
3. Drag a track from Windows Explorer onto Deck A.
4. Have ArcLive analyze its tempo.
5. Automatically prepare the track for the global BPM.
6. Press Play.
7. Drag another track onto Deck B.
8. Have the second track analyzed and warped.
9. Preview Deck B through headphones.
10. Synchronize Deck B with Deck A.
11. Adjust LOW, MID, and HIGH EQ.
12. Mix using channel faders and the crossfader.
13. Continue playing without interruptions.

Example:

Global tempo: 146 BPM.

Deck A original tempo: 138 BPM.

Deck B original tempo: 150 BPM.

Both decks should play at 146 BPM with pitch preservation enabled.

The application must distinguish between tempo matching and beat-phase synchronization.

Automatic warping must not silently assume an unreliable beat grid is correct.

---

# 5. UI DESIGN PHILOSOPHY

The interface must feel like a professional application designed by Apple.

Think:

- Logic Pro.
- Final Cut Pro.
- Premium macOS utility applications.
- Minimal professional audio software.

But do not copy their interfaces directly.

The result must be original.

## 5.1 Visual Personality

- Elegant.
- Restrained.
- Precise.
- Dark.
- Minimal.
- Professional.
- Technically sophisticated.
- Calm.
- Highly responsive.

Avoid:

- Neon cyberpunk aesthetics.
- Excessive gradients.
- Fake metal knobs.
- Oversized dashboard cards.
- Large decorative shadows.
- Excessive glass effects.
- Giant typography.
- Unnecessary animations.
- Generic dashboard layouts.
- Cluttered hardware-controller imitation.

## 5.2 Color Palette

Default dark theme:

| Element | Color |
|---|---|
| Background | #111316 |
| Main panel | #1B1E23 |
| Elevated panel | #24282E |
| Borders | #343941 |
| Primary text | #F2F3F5 |
| Secondary text | #989FA9 |
| Deck A | #62B9E8 |
| Deck B | #E6AD70 |
| Synced | #71C9A2 |
| Warning | #E5B96D |
| Error | #E87D7D |

Use a centralized theme system.

## 5.3 Typography

Use a clean native system font.

Prefer:

- Segoe UI on Windows.
- Native platform fonts when supporting other operating systems.

Use tabular numerals for BPM and time displays.

Keep typography compact.

## 5.4 Spacing

Use a 4 px spacing grid.

Use consistent control sizes.

Avoid excessive whitespace.

This is a live performance instrument, not a marketing website.

---

# 6. MAIN WINDOW DESIGN

The application should open into a professional DJ workspace.

Target 1920 × 1080.

Support smaller laptop displays.

## 6.1 Main Layout

```text
┌──────────────────────────────────────────────────────────────┐
│ ARCLIVE       MASTER BPM 146.00        AUDIO       SETTINGS   │
├────────────────────────────┬─────────────────────────────────┤
│ DECK A                     │ DECK B                          │
│                            │                                 │
│ Track Title                │ Track Title                     │
│ Artist / BPM / Duration    │ Artist / BPM / Duration         │
│                            │                                 │
│ ┌────────────────────────┐ │ ┌─────────────────────────────┐ │
│ │                        │ │ │                             │ │
│ │       WAVEFORM         │ │ │          WAVEFORM           │ │
│ │                        │ │ │                             │ │
│ └────────────────────────┘ │ └─────────────────────────────┘ │
│                            │                                 │
│ PLAY  CUE  SYNC  LOOP      │ PLAY  CUE  SYNC  LOOP          │
├────────────────────────────┴─────────────────────────────────┤
│                                                              │
│       DECK A             MIXER              DECK B           │
│                                                              │
│    HIGH MID LOW                       LOW MID HIGH           │
│                                                              │
│    FILTER  FADER       CROSSFADER      FADER FILTER          │
│                                                              │
│                     MASTER OUTPUT                            │
├──────────────────────────────────────────────────────────────┤
│ LIBRARY       SEARCH TRACKS                IMPORT FOLDER     │
├──────────────────────────────────────────────────────────────┤
│ TITLE                  ARTIST        BPM      DURATION       │
│ Track 1                Artist        146      07:42          │
│ Track 2                Artist        138      08:16          │
│ Track 3                Artist        150      06:54          │
└──────────────────────────────────────────────────────────────┘
```

This is a layout guide, not a rigid pixel specification.

Refine the proportions.

## 6.2 Window Behavior

Support:

- Resize.
- Maximize.
- Fullscreen.
- Persistent window dimensions.
- Collapsible library.
- Adjustable library height.
- High-DPI scaling.

Avoid clipping and overlapping controls.

---

# 7. MANDATORY WINDOWS DRAG-AND-DROP

**Drag-and-drop is a first-class product requirement, not an optional enhancement.**

This must work with actual files from Windows Explorer.

## 7.1 External File Drop

The user must be able to:

1. Open Windows Explorer.
2. Select an audio file.
3. Drag it over ArcLive.
4. Hover over Deck A or Deck B.
5. Release the mouse.
6. Have that exact track loaded into the selected deck.

Supported initial formats:

- WAV.
- AIFF.
- FLAC.
- MP3.

Verify decoder support for each format.

## 7.2 Drag-Over Feedback

When dragging an audio file over Deck A:

- Highlight Deck A's border in blue.
- Slightly brighten its background.
- Display a small message such as "Drop to load Deck A".
- Keep all other controls visible.

When hovering over Deck B:

- Highlight Deck B in amber.
- Display "Drop to load Deck B".

The highlight must follow the actual drop target.

Do not highlight both decks simultaneously.

## 7.3 Native Drop Event Handling

Use the selected GUI framework's native drag-and-drop capabilities.

If native Windows integration is required, use a safe and maintainable Rust abstraction.

Verify whether the framework provides actual file paths.

Do not simulate drag-and-drop using click events.

Do not require users to copy paths manually.

Do not route file drops through a browser.

## 7.4 Drop Position

The destination must be determined by the actual pointer position.

Dropping over Deck A loads Deck A.

Dropping over Deck B loads Deck B.

Dropping over the library imports the track into the library.

Do not guess the destination.

## 7.5 Multiple Files

Support multiple-file drops.

Behavior:

- Dropping multiple files on the library imports all valid files.
- Dropping multiple files onto one deck must not silently replace tracks repeatedly.
- Provide a clear policy for multi-file deck drops, such as rejecting the operation with a concise explanation.

## 7.6 Unsupported Files

If the user drops an unsupported file:

- Do not crash.
- Do not alter the currently playing deck.
- Display a clear message.
- Explain which formats are supported.

## 7.7 Load Protection

If the target deck is playing:

- Do not replace the track automatically.
- Respect load-protection settings.
- Require explicit confirmation or block the operation.

Default behavior should prioritize live-performance safety.

## 7.8 Dragging from the Internal Library

Users must also be able to drag tracks from ArcLive's own library onto either deck.

Use the same target-highlighting behavior.

## 7.9 Drag-and-Drop Acceptance Tests

Test:

- WAV from Explorer to Deck A.
- MP3 from Explorer to Deck B.
- FLAC to library.
- Multiple files to library.
- Unsupported file.
- File dropped outside a valid target.
- File dropped onto an active deck.
- Internal library row dragged to Deck A.
- Internal library row dragged to Deck B.
- Long file paths.
- Unicode filenames.
- Files from directories containing spaces.

**Do not consider the drag-and-drop feature complete until actual Windows Explorer file drops have been verified.**

---

# 8. TRACK LOADING PIPELINE

When a valid file is dropped onto a deck:

1. Validate the path and format.
2. Open the audio file.
3. Extract metadata.
4. Initialize decoding.
5. Generate waveform summary data.
6. Analyze BPM.
7. Estimate beat-grid positions.
8. Estimate analysis confidence.
9. Prepare tempo adaptation.
10. Initialize playback buffers.
11. Update deck state.
12. Display Ready when genuinely ready.

Do not block the UI.

Do not interrupt the other deck.

Use background workers for expensive tasks.

## 8.1 Loading States

- Empty.
- Loading.
- Analyzing.
- Preparing warp.
- Ready.
- Playing.
- Paused.
- Error.

Show subtle progress feedback.

Do not use large modal loading screens.

## 8.2 Caching

If the track was previously analyzed:

- Retrieve cached metadata.
- Retrieve cached waveform data.
- Retrieve beat grid.
- Retrieve warp corrections.
- Avoid unnecessary reanalysis.

Validate that the underlying file has not changed.

---

# 9. DECK DESIGN

Each deck must include:

- Track header.
- Waveform.
- Overview waveform.
- Playback time.
- Original BPM.
- Effective BPM.
- Sync indicator.
- Play/Pause.
- Cue.
- Loop.
- Quantize.

## 9.1 Empty Deck

An empty deck should show a restrained drop target.

Example:

**DECK A**

"Drag an audio track here"

Below:

"or click to browse"

Use a subtle dashed or low-contrast border.

Do not use oversized illustrations.

## 9.2 Loaded Deck

Once loaded:

- Replace the empty state with track metadata.
- Render waveform.
- Display detected BPM.
- Show synchronization readiness.
- Enable valid transport controls.

## 9.3 Deck Status

Use clear state indicators.

Do not indicate beat synchronization simply because BPM values match.

---

# 10. WAVEFORM VISUALIZATION

Waveforms must be high-quality and smooth.

## 10.1 Required Elements

- Scrolling waveform.
- Stable playhead.
- Beat markers.
- Bar markers.
- Cue markers.
- Loop regions.
- Overview waveform.
- Zoom controls.

## 10.2 Native Rendering

Use the selected Rust-native GUI framework's rendering facilities.

If custom GPU rendering is required, implement an appropriate native rendering integration.

Do not use HTML Canvas or WebGL.

Waveform visualization must not perform audio processing.

## 10.3 Performance

Precompute waveform summaries.

Avoid repeatedly decoding audio for rendering.

Keep waveform updates independent of the real-time audio callback.

## 10.4 Visual Design

Deck A waveform: restrained blue.

Deck B waveform: restrained amber.

Use subtle grid lines.

Avoid excessive brightness.

---

# 11. MASTER BPM

Provide a global BPM control at the top center.

Example:

MASTER BPM

146.00

Interactions:

- Click to edit.
- Double-click for exact entry.
- Vertical drag to adjust.
- Keyboard arrows.
- Shift for fine adjustment.
- Tap tempo.

All synchronized decks must follow the global BPM.

Tempo changes must be smoothed where necessary.

---

# 12. AUTOMATIC BPM ANALYSIS

Implement in Rust.

Pipeline:

1. Decode audio.
2. Preprocess signal.
3. Detect onsets.
4. Estimate tempo candidates.
5. Track beats.
6. Estimate downbeats.
7. Generate beat grid.
8. Compute confidence.
9. Cache results.

Support manual correction.

Provide:

- BPM input.
- Half-time correction.
- Double-time correction.
- Grid offset.
- First downbeat selection.

Persist user corrections.

---

# 13. AUTOMATIC WARPING

This is the defining ArcLive feature.

## 13.1 Required Behavior

Global BPM: 146.

Track original BPM: 138.

On loading, ArcLive prepares playback at 146 BPM.

Preserve original musical pitch by default.

The track should not begin playing automatically unless the user explicitly enables an auto-play preference.

## 13.2 Two Warp Modes

### Constant-Tempo Warp

For stable electronic music.

Apply a consistent time-stretch ratio.

Implement this first.

### Elastic Warp

For tracks with variable timing.

Support editable warp markers and source-time-to-musical-time mapping.

Implement after constant-tempo warping is reliable.

## 13.3 Pitch Preservation

Use a verified time-stretching implementation.

Evaluate real-time suitability and licensing.

Do not substitute simple resampling for pitch-preserving warping.

## 13.4 Beat Alignment

Tempo matching and phase alignment are separate.

Implement both.

A track must be aligned to the global musical clock when sync is engaged.

## 13.5 Manual Override

Allow users to:

- Correct BPM.
- Shift grid.
- Set downbeat.
- Add warp markers.
- Edit warp markers.
- Reset warp mapping.

Persist corrections.

---

# 14. MIXER DESIGN

Create a native professional DJ mixer.

## 14.1 Three-Band EQ

Each deck:

- HIGH.
- MID.
- LOW.

Actual DSP must be implemented in Rust.

Controls must not be visual simulations.

## 14.2 EQ Knobs

Design:

- Flat circular controls.
- Thin value arc.
- Position indicator.
- Compact label.
- Precise tooltip.

Interactions:

- Drag vertically.
- Shift-drag for fine adjustment.
- Double-click reset.
- Keyboard operation.

## 14.3 Channel Gain

Independent gain for each deck.

Include clipping indicators.

## 14.4 Channel Faders

Vertical volume faders.

Use appropriate gain curves.

Smooth DSP parameter changes.

## 14.5 Crossfader

A prominent horizontal crossfader.

Left = Deck A.

Right = Deck B.

Center = both.

Support:

- Linear.
- Equal-power.
- Sharp cut.

Use smooth gain interpolation.

## 14.6 Filters

Each deck gets a combined filter.

Left: low-pass.

Center: neutral.

Right: high-pass.

Implement stable DSP.

## 14.7 Meters

Provide:

- Deck A meter.
- Deck B meter.
- Master meter.
- Peak indicators.
- Clipping warnings.

---

# 15. HEADPHONE CUE

Implement professional headphone monitoring.

Support:

- Master stereo output.
- Cue stereo output.
- Deck A cue.
- Deck B cue.
- Cue/master blend.
- Headphone volume.

Use explicit audio channel mapping.

If an interface lacks separate cue outputs, communicate the limitation.

Do not silently mix cue audio into the master output.

---

# 16. AUDIO ENGINE

Implement a dedicated Rust audio engine.

Suggested modules:

```text
src/
  main.rs
  app.rs

  audio/
    mod.rs
    engine.rs
    device.rs
    callback.rs
    decoder.rs
    buffers.rs
    clock.rs

  deck/
    mod.rs
    deck.rs
    playback.rs
    transport.rs
    loop_engine.rs

  analysis/
    mod.rs
    bpm.rs
    onset.rs
    beat_grid.rs
    waveform.rs

  warp/
    mod.rs
    stretch.rs
    mapping.rs
    markers.rs
    sync.rs

  mixer/
    mod.rs
    eq.rs
    filter.rs
    gain.rs
    crossfader.rs
    routing.rs
    meter.rs

  library/
    mod.rs
    database.rs
    scanner.rs
    metadata.rs
    cache.rs

  ui/
    mod.rs
    window.rs
    deck.rs
    mixer.rs
    library.rs
    settings.rs

  settings/
    mod.rs
    config.rs

  diagnostics/
    mod.rs
    metrics.rs
```

Adapt this structure to the chosen GUI framework and existing repository.

Do not create unnecessary empty modules.

---

# 17. REAL-TIME AUDIO SAFETY

The audio callback must avoid:

- File I/O.
- Database access.
- Blocking mutexes.
- Network operations.
- Unbounded allocations.
- Synchronous logging.
- UI calls.
- Track analysis.
- Waiting on background workers.

Use:

- Preallocated buffers.
- Bounded queues.
- Appropriate lock-free communication.
- Background decoding.
- Worker-thread analysis.
- Predictable DSP.
- Safe device shutdown.

The UI must not interrupt audio playback.

---

# 18. MUSIC LIBRARY

Implement a native table-style library.

Columns:

- Title.
- Artist.
- BPM.
- Duration.
- Key.
- Format.
- Analysis status.

Support:

- Search.
- Sorting.
- Folder import.
- File import.
- Drag-and-drop.
- Playlists.
- Analysis caching.

Use SQLite.

Large libraries must remain responsive.

---

# 19. SETTINGS

Implement a native settings panel.

Sections:

- Audio.
- Performance.
- Library.
- Appearance.
- Keyboard.
- Advanced.

Audio settings:

- Device.
- Backend.
- Sample rate.
- Buffer size.
- Master channels.
- Cue channels.

Performance settings:

- Quantization.
- Sync behavior.
- Load protection.
- Crossfader curve.
- Warp mode.

Appearance:

- Theme.
- Waveform colors.
- UI scaling.
- Compact mode.

---

# 20. KEYBOARD AND MOUSE

Support precise desktop interaction.

Mouse:

- Drag-and-drop.
- Knob dragging.
- Fader dragging.
- Waveform seeking.
- Context menus.
- Tooltips.

Keyboard:

- Deck playback shortcuts.
- Cue shortcuts.
- Load selected track.
- Focus search.
- Master BPM editing.
- Settings access.

Avoid shortcuts that can accidentally interrupt a live performance.

---

# 21. TESTING REQUIREMENTS

## Native UI

Test:

- Window launch.
- Resizing.
- DPI scaling.
- File dialogs.
- Windows Explorer drag-and-drop.
- Internal drag-and-drop.
- Knob interaction.
- Crossfader movement.

## Audio

Test:

- Decoding.
- Playback.
- EQ.
- Filters.
- Mixing.
- Cue routing.
- Audio device changes.

## Warping

Test:

- 138 → 146 BPM.
- 150 → 146 BPM.
- Pitch preservation.
- Beat synchronization.
- Tempo changes.
- Warp marker corrections.

## Performance

Measure:

- Audio callback duration.
- Underruns.
- CPU usage.
- Memory usage.
- Track loading time.
- Waveform rendering performance.

Do not fabricate benchmark results.

---

# 22. IMPLEMENTATION PHASES

## Phase 1 — Native Rust Application

Create:

- Cargo project.
- Native GUI.
- Main window.
- Theme.
- Global toolbar.
- Two deck panels.
- Mixer UI.
- Library UI.
- Settings shell.

Produce a working Windows executable.

## Phase 2 — Native Drag-and-Drop

Implement:

- Explorer file drops.
- Deck target detection.
- Hover feedback.
- Internal library dragging.
- File validation.
- Loading states.
- Load protection.

Verify with actual Windows Explorer.

## Phase 3 — Audio Playback

Implement:

- Audio device selection.
- Decoding.
- Deck playback.
- Master output.
- Play/pause.
- Seek.
- Basic metering.

## Phase 4 — Mixer DSP

Implement:

- EQ.
- Filters.
- Channel faders.
- Crossfader.
- Cue routing.

## Phase 5 — BPM and Warping

Implement:

- BPM detection.
- Beat grid.
- Global clock.
- Pitch-preserving time stretching.
- Automatic tempo matching.
- Beat alignment.
- Quantization.

## Phase 6 — Elastic Warp

Implement:

- Warp markers.
- Variable-tempo mapping.
- Manual correction.
- Persistence.

## Phase 7 — Library and Professional Workflow

Implement:

- SQLite library.
- Search.
- Playlists.
- Hot cues.
- Loops.
- Keyboard shortcuts.

## Phase 8 — Production

Implement:

- Performance optimization.
- Audio stress tests.
- UI polish.
- Error handling.
- Packaging.
- Windows release build.

---

# 23. CODE QUALITY RULES

- Rust-first.
- No web frontend.
- No Tauri.
- No Electron.
- No JavaScript audio engine.
- No fake DSP.
- No placeholder playback.
- No fabricated analysis results.
- Preserve existing working code.
- Do not truncate source files.
- Do not silently remove functionality.
- Use modular architecture.
- Document technical decisions.
- Add meaningful tests.
- Build after significant changes.
- Fix compilation errors before proceeding.
- Validate audio functionality.
- Verify drag-and-drop on Windows.

---

# 24. FIRST ACTION FOR CURSOR

Before coding:

1. Inspect the repository.
2. Identify existing functionality.
3. Identify any Tauri, React, or web-based architecture.
4. Plan the migration to native Rust.
5. Preserve reusable Rust audio and DSP code.
6. Do not blindly delete working components.
7. Evaluate Slint and alternative native GUI frameworks.
8. Verify Windows drag-and-drop support.
9. Verify audio dependencies.
10. Verify licensing requirements.
11. Create architecture documentation.
12. Establish implementation milestones.

If the repository already contains Tauri or React code, do not continue building that frontend.

Replace the web UI architecture with a native Rust-compatible GUI while preserving working engine functionality.

Create:

```text
docs/
  PRODUCT_SPEC.md
  NATIVE_UI_DESIGN.md
  AUDIO_ARCHITECTURE.md
  DRAG_DROP_SPEC.md
  WARP_ENGINE.md
  IMPLEMENTATION_PLAN.md
  TESTING_STRATEGY.md
  DEVELOPMENT_LOG.md
```

After planning, begin implementation.

Do not stop after writing documentation.

Build the native application shell.

Implement real drag-and-drop.

Compile and test the executable.

---

# 25. FINAL ACCEPTANCE CRITERIA

ArcLive DJ is successful when:

1. `ArcLive.exe` launches directly on Windows.
2. No browser, webview, or local server is required.
3. The interface is native and professionally designed.
4. A WAV file can be dragged from Explorer to Deck A.
5. An MP3 can be dragged from Explorer to Deck B.
6. The correct deck receives the correct file.
7. Track metadata appears.
8. Waveforms render.
9. BPM analysis runs.
10. Tracks automatically prepare for master BPM.
11. Playback works through the selected audio device.
12. Pitch is preserved during tempo changes.
13. Beat synchronization works.
14. Three-band EQ processes real audio.
15. Channel faders process real audio.
16. The crossfader mixes the decks.
17. Headphone cue routing works on supported devices.
18. The library imports and searches tracks.
19. Playback remains stable during background analysis.
20. The application can be packaged and launched reliably.

No simulated audio behavior counts as a completed feature.

---

# 26. FINAL PRODUCT VISION

Imagine a native Rust application with the visual refinement of Apple's professional software and the performance workflow of a modern DJ system.

The user opens ArcLive.

They see two elegant decks, a precise global BPM control, beautiful waveforms, and a clean professional mixer.

They drag a track from Windows Explorer onto Deck A.

ArcLive loads it.

Analyzes it.

Warps it to the current BPM.

The user presses Play.

They drag another track onto Deck B.

ArcLive prepares that track automatically.

They preview it through headphones.

They synchronize it.

They adjust the three EQ bands.

They move the crossfader.

The transition is smooth.

No DAW configuration.

No manual tempo matching.

No browser technology.

No unnecessary complexity.

**ArcLive DJ**

**100% Native Rust. Standalone Windows EXE. Drag. Warp. Sync. Mix.**

Begin with repository inspection, native Rust architecture selection, and implementation of the real Windows desktop application. Prioritize native drag-and-drop, reliable audio playback, and genuine DSP over superficial interface completion.