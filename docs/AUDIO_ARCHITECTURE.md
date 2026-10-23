# Audio Architecture

## Devices

- **Default:** WASAPI via `cpal` (shared mode).
- **ASIO:** Not in default redistributable (Steinberg SDK licensing). Settings UI states WASAPI-only for the shipping build.
- **Cue:** Second output device when available. Cue is never silently mixed into master; UI shows the limitation when only one device exists.

## Threading

```
Decoder worker → Stretch worker → Bounded ring (rtrb)
                                      ↓
UI thread ──atomics──→ cpal callback → Master / Cue devices
```

Callback rules (hard):

- No file I/O, SQLite, network, UI, analysis, unbounded alloc, blocking mutexes, sync logging.
- Preallocated buffers; smoothed parameter ramps; atomics for meters and transport flags.

## Libraries

| Crate | Role |
|---|---|
| `cpal` | Device I/O |
| `symphonia` | Decode WAV/AIFF/FLAC/MP3 |
| `rtrb` | Lock-free rings into callback |
| `rubato` | Sample-rate conversion only (not time stretch) |
| `realfft` | Analysis FFT |
| `rusqlite` (bundled) | Library DB |

## Time stretching

Default: in-process WSOLA (Rust) for constant-tempo pitch-preserving warp. Rubber Band is optional later (GPL or commercial license) and is not linked by default.

## Licensing notes

- Slint: royalty-free desktop + attribution.
- Rubber Band (if enabled later): GPL-2+ or commercial.
- ASIO SDK: not redistributed.