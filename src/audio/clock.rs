use std::sync::atomic::{AtomicU64, Ordering};

/// Global musical clock in beats (fixed-point: beats * 1e6 as u64).
pub struct MasterClock {
    beats_micros: AtomicU64,
    bpm_micros: AtomicU64,
}

impl MasterClock {
    pub fn new(bpm: f64) -> Self {
        Self {
            beats_micros: AtomicU64::new(0),
            bpm_micros: AtomicU64::new((bpm * 1_000_000.0) as u64),
        }
    }

    pub fn set_bpm(&self, bpm: f64) {
        self.bpm_micros
            .store((bpm.clamp(40.0, 300.0) * 1_000_000.0) as u64, Ordering::Relaxed);
    }

    pub fn bpm(&self) -> f64 {
        self.bpm_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    pub fn beats(&self) -> f64 {
        self.beats_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Advance clock by `frames` at `sample_rate` when transport is running.
    pub fn advance(&self, frames: u32, sample_rate: u32) {
        let bpm = self.bpm();
        let beat_delta = (frames as f64 / sample_rate as f64) * (bpm / 60.0);
        let add = (beat_delta * 1_000_000.0) as u64;
        self.beats_micros.fetch_add(add, Ordering::Relaxed);
    }

    pub fn reset(&self) {
        self.beats_micros.store(0, Ordering::Relaxed);
    }
}