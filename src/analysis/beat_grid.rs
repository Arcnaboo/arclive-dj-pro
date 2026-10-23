use crate::analysis::bpm::BpmAnalysis;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatGrid {
    pub bpm: f64,
    pub offset_secs: f64,
    pub confidence: f64,
    /// User corrections
    pub manual_bpm: Option<f64>,
    pub manual_offset: Option<f64>,
}

impl BeatGrid {
    pub fn from_analysis(a: &BpmAnalysis) -> Self {
        Self {
            bpm: a.bpm,
            offset_secs: a.offset_secs,
            confidence: a.confidence,
            manual_bpm: None,
            manual_offset: None,
        }
    }

    pub fn effective_bpm(&self) -> f64 {
        self.manual_bpm.unwrap_or(self.bpm)
    }

    pub fn effective_offset(&self) -> f64 {
        self.manual_offset.unwrap_or(self.offset_secs)
    }

    pub fn beat_at_time(&self, secs: f64) -> f64 {
        let bpm = self.effective_bpm();
        let off = self.effective_offset();
        (secs - off) * bpm / 60.0
    }

    pub fn time_at_beat(&self, beat: f64) -> f64 {
        let bpm = self.effective_bpm();
        let off = self.effective_offset();
        off + beat * 60.0 / bpm
    }

    pub fn apply_half_time(&mut self) {
        let b = self.effective_bpm() * 0.5;
        self.manual_bpm = Some(b);
    }

    pub fn apply_double_time(&mut self) {
        let b = self.effective_bpm() * 2.0;
        self.manual_bpm = Some(b);
    }

    pub fn shift_offset(&mut self, delta_secs: f64) {
        let o = self.effective_offset() + delta_secs;
        self.manual_offset = Some(o);
    }
}