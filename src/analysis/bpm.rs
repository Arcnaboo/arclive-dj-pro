use crate::analysis::onset::onset_envelope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmAnalysis {
    pub bpm: f64,
    pub confidence: f64,
    pub beat_period_secs: f64,
    pub offset_secs: f64,
}

pub fn analyze_bpm(samples: &[f32], sample_rate: u32) -> BpmAnalysis {
    let hop = 512usize;
    let env = onset_envelope(samples, sample_rate, hop);
    if env.len() < 32 {
        return BpmAnalysis {
            bpm: 120.0,
            confidence: 0.0,
            beat_period_secs: 0.5,
            offset_secs: 0.0,
        };
    }

    let hop_secs = hop as f64 / sample_rate as f64;
    let min_bpm = 90.0;
    let max_bpm = 180.0;
    let min_lag = (60.0 / max_bpm / hop_secs).round() as usize;
    let max_lag = (60.0 / min_bpm / hop_secs).round() as usize;

    let mut best_lag = min_lag;
    let mut best_score = f32::MIN;
    let mut scores = Vec::new();

    for lag in min_lag..=max_lag.min(env.len() / 2) {
        let mut corr = 0.0f32;
        let mut norm = 0.0f32;
        for i in 0..env.len() - lag {
            corr += env[i] * env[i + lag];
            norm += env[i] * env[i];
        }
        let score = if norm > 1e-8 { corr / norm } else { 0.0 };
        scores.push((lag, score));
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }

    let period_secs = best_lag as f64 * hop_secs;
    let mut bpm = 60.0 / period_secs;

    // Prefer electronic music range; check half/double
    let candidates = [bpm, bpm * 2.0, bpm * 0.5];
    let mut best = bpm;
    let mut best_dist = f64::MAX;
    for c in candidates {
        if (90.0..=180.0).contains(&c) {
            let dist = (c - 140.0).abs(); // soft prior toward trance tempos
            if dist < best_dist {
                best_dist = dist;
                best = c;
            }
        }
    }
    bpm = best;

    // Estimate offset: first strong peak
    let threshold = env.iter().cloned().fold(0.0f32, f32::max) * 0.4;
    let mut offset_secs = 0.0;
    for (i, &v) in env.iter().enumerate() {
        if v >= threshold {
            offset_secs = i as f64 * hop_secs;
            break;
        }
    }

    let confidence = (best_score as f64).clamp(0.0, 1.0);
    BpmAnalysis {
        bpm,
        confidence,
        beat_period_secs: 60.0 / bpm,
        offset_secs,
    }
}

pub fn half_time(bpm: f64) -> f64 {
    bpm * 0.5
}

pub fn double_time(bpm: f64) -> f64 {
    bpm * 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn detects_click_tempo() {
        let sr = 44100u32;
        let bpm = 146.0;
        let period = (60.0 / bpm * sr as f64) as usize;
        let seconds = 20;
        let frames = sr as usize * seconds;
        let mut samples = vec![0.0f32; frames * 2];
        for beat in 0..(seconds as f64 * bpm / 60.0) as usize {
            let pos = beat * period;
            if pos + 64 < frames {
                for i in 0..64 {
                    let s = (2.0 * PI * 1000.0 * i as f32 / sr as f32).sin()
                        * (1.0 - i as f32 / 64.0);
                    samples[(pos + i) * 2] = s;
                    samples[(pos + i) * 2 + 1] = s;
                }
            }
        }
        let a = analyze_bpm(&samples, sr);
        assert!(
            (a.bpm - bpm).abs() < 3.0 || (a.bpm - bpm * 0.5).abs() < 3.0 || (a.bpm - bpm * 2.0).abs() < 3.0,
            "got bpm {}",
            a.bpm
        );
    }
}