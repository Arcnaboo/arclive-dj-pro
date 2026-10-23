# Warp Engine

## Modes

### Constant-tempo warp (phase 5, first)

Stable electronic music: one time-stretch ratio = `master_bpm / track_bpm`. Pitch preserved via WSOLA. Default on load; auto-play off unless preference set.

### Elastic warp (phase 6)

Editable warp markers mapping source time ↔ musical time for variable timing. Persist corrections.

## Tempo match vs phase sync

- **Tempo match:** stretch so effective BPM equals master BPM.
- **Phase sync:** align beats to the global musical clock when Sync is engaged. Matching BPM alone does not imply sync.

## Analysis pipeline

Decode → preprocess → onset → tempo candidates → beat track → downbeat → beat grid → confidence → cache (path + size + mtime).

Manual: BPM entry, half/double time, grid offset, first downbeat. Persist user corrections.

## Stretch implementation

Primary: Rust WSOLA suitable for small DJ ratios (e.g. 138→146, 150→146). Optional Rubber Band path documented but not default-linked.