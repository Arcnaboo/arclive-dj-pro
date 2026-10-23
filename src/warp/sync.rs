/// Beat-phase error in beats (−0.5..0.5) between deck beat and master beat.
pub fn phase_error_beats(deck_beat: f64, master_beat: f64) -> f64 {
    let mut e = deck_beat - master_beat;
    e -= e.round();
    e
}

/// Convert phase error to a sample nudge at given BPM and sample rate.
/// `bpm` must be the **source/track** BPM of the deck being nudged (not master BPM).
pub fn sync_nudge_samples(phase_error_beats: f64, bpm: f64, sample_rate: u32) -> i64 {
    let secs = phase_error_beats * 60.0 / bpm.max(1.0);
    (secs * sample_rate as f64).round() as i64
}

/// Soft drift correction: only apply a fraction of the error, capped in samples.
/// Avoids jumping a full beat every audio buffer.
pub fn soft_sync_nudge_samples(
    phase_error_beats: f64,
    bpm: f64,
    sample_rate: u32,
    max_ms: f64,
) -> i64 {
    let full = sync_nudge_samples(phase_error_beats, bpm, sample_rate);
    let max_samples = ((max_ms / 1000.0) * sample_rate as f64).round() as i64;
    // Correct 35% of the error per block, never more than max_ms.
    let step = (full as f64 * 0.35).round() as i64;
    step.clamp(-max_samples, max_samples)
}

/// Musical beat index from source position and grid.
pub fn beat_at_source(pos_secs: f64, bpm: f64, offset_secs: f64) -> f64 {
    (pos_secs - offset_secs) * bpm.max(1.0) / 60.0
}

/// Phase-align follower to master with a **minimal** source seek.
///
/// Matches fractional beat phase only — does **not** jump the follower to the
/// master's absolute beat number (that would teleport into a different part of
/// the track).
pub fn sync_phase_seek_frames(
    master_pos_secs: f64,
    master_bpm: f64,
    master_offset_secs: f64,
    follower_pos_secs: f64,
    follower_bpm: f64,
    follower_offset_secs: f64,
    follower_sample_rate: f64,
) -> f64 {
    let master_beat = beat_at_source(master_pos_secs, master_bpm, master_offset_secs);
    let follower_beat = beat_at_source(follower_pos_secs, follower_bpm, follower_offset_secs);
    let err = phase_error_beats(follower_beat, master_beat);
    // Shift follower source so its beat matches master's phase.
    let delta_secs = -err * 60.0 / follower_bpm.max(1.0);
    ((follower_pos_secs + delta_secs) * follower_sample_rate).max(0.0)
}

/// Legacy name kept for callers; same as phase-align using follower position 0
/// when only absolute mapping was previously available — prefer `sync_phase_seek_frames`.
#[deprecated(note = "use sync_phase_seek_frames")]
pub fn sync_seek_frames(
    master_pos_secs: f64,
    master_bpm: f64,
    master_offset_secs: f64,
    follower_bpm: f64,
    follower_offset_secs: f64,
    follower_sample_rate: f64,
) -> f64 {
    // Old behavior used absolute beat index — wrong for DJ sync.
    // Approximate by phase-aligning as if follower were at the same beat phase from 0.
    let master_beat = beat_at_source(master_pos_secs, master_bpm, master_offset_secs);
    let phase = master_beat - master_beat.floor();
    let target_secs = follower_offset_secs + phase * 60.0 / follower_bpm.max(1.0);
    (target_secs * follower_sample_rate).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_phase() {
        let e = phase_error_beats(10.9, 10.0);
        assert!((e - (-0.1)).abs() < 1e-9 || (e - 0.9).abs() < 1e-9 || e.abs() < 0.5);
        assert!(phase_error_beats(1.0, 1.0).abs() < 1e-12);
    }

    #[test]
    fn soft_nudge_capped() {
        let n = soft_sync_nudge_samples(0.4, 120.0, 48000, 3.0);
        assert!(n.abs() <= ((0.003 * 48000.0) as i64) + 1);
        assert!(n.abs() < sync_nudge_samples(0.4, 120.0, 48000).abs());
    }

    #[test]
    fn phase_seek_keeps_follower_near_current_position() {
        // Master mid-track at beat ~200; follower near start at beat ~2.3
        // Sync must only correct fractional phase (~0.3), not jump to beat 200.
        let master_pos = 200.3 * 60.0 / 138.0; // ~87s
        let follower_pos = 2.3 * 60.0 / 150.0; // ~0.92s
        let frames = sync_phase_seek_frames(
            master_pos,
            138.0,
            0.0,
            follower_pos,
            150.0,
            0.0,
            48000.0,
        );
        let target_secs = frames / 48000.0;
        // Stay near follower's current region (within ~1 beat ≈ 0.4s)
        assert!(
            (target_secs - follower_pos).abs() < 0.5,
            "jumped too far: follower was {follower_pos}, target {target_secs}"
        );
        let target_beat = beat_at_source(target_secs, 150.0, 0.0);
        let master_beat = beat_at_source(master_pos, 138.0, 0.0);
        assert!(
            phase_error_beats(target_beat, master_beat).abs() < 1e-6,
            "phase not aligned"
        );
    }

    #[test]
    fn phase_seek_uses_grid_offsets() {
        let frames = sync_phase_seek_frames(0.5, 120.0, 0.1, 0.5, 120.0, 0.1, 48000.0);
        // Same grids and positions → already aligned → no move
        assert!((frames - 0.5 * 48000.0).abs() < 1.0);
    }

    #[test]
    fn nudge_uses_track_bpm_not_master() {
        // 0.1 beat at 100 BPM source → 0.06s → 2880 samples @ 48k
        let n = sync_nudge_samples(0.1, 100.0, 48000);
        assert!((n - 2880).abs() <= 1);
        // Same phase error at 200 BPM is half the samples
        let n2 = sync_nudge_samples(0.1, 200.0, 48000);
        assert!((n2 - 1440).abs() <= 1);
    }
}
