/// Beat-phase error in beats (−0.5..0.5) between deck beat and master beat.
pub fn phase_error_beats(deck_beat: f64, master_beat: f64) -> f64 {
    let mut e = deck_beat - master_beat;
    e -= e.round();
    e
}

/// Convert phase error to a sample nudge at given BPM and sample rate.
pub fn sync_nudge_samples(phase_error_beats: f64, bpm: f64, sample_rate: u32) -> i64 {
    let secs = phase_error_beats * 60.0 / bpm;
    (secs * sample_rate as f64).round() as i64
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
}