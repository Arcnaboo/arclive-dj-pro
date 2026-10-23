# Testing Strategy

## Automated

- Unit tests for format validation, hit-testing, EQ/filter coefficients, crossfader curves, WSOLA ratio math, BPM candidate scoring, library SQL.
- Integration tests for decode of fixture WAV/FLAC where feasible headless.
- Do not fabricate benchmark numbers.

## Manual (Windows)

### UI

Window launch, resize, DPI, file dialogs, Explorer DnD, internal DnD, knobs, crossfader.

### Audio

Decode, playback, EQ, filters, mix, cue routing, device change.

### Warp

138→146, 150→146, pitch preservation, beat sync, tempo change, marker edit.

### Performance

Callback duration, underruns, CPU/memory, load time, waveform FPS — measure and record in DEVELOPMENT_LOG.

## Explorer DnD checklist

See DRAG_DROP_SPEC.md acceptance tests.