//! Three-band DJ isolator EQ (low shelf / peaking mid / high shelf).
//!
//! UI 0..1 mapping (center 0.5 = flat):
//! - Above center: up to +6 dB boost
//! - Below center: cut toward kill (silence) at 0.0

#[derive(Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    fn low_shelf(sample_rate: f32, freq: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * (2.0f32).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn high_shelf(sample_rate: f32, freq: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * (2.0f32).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn peaking(sample_rate: f32, freq: f32, q: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }
}

/// Isolator curve: 0 = kill, 0.5 = 0 dB, 1.0 = +6 dB.
/// Below center the band gain falls toward silence (not a mild −12 dB cut).
pub fn isolator_band_gain(ui: f32) -> f32 {
    let u = ui.clamp(0.0, 1.0);
    if u <= 0.5 {
        // Smooth kill: cubic ease so the last part of the throw is decisive.
        let t = u / 0.5;
        t * t * t
    } else {
        let t = (u - 0.5) / 0.5;
        10f32.powf((t * 6.0) / 20.0) // 0..+6 dB
    }
}

/// Approximate dB label for the knob (−∞ at kill, 0 at center, +6 at top).
pub fn isolator_db_label(ui: f32) -> f32 {
    let g = isolator_band_gain(ui);
    if g < 1e-4 {
        -96.0
    } else {
        20.0 * g.log10()
    }
}

pub struct ThreeBandEq {
    low: Biquad,
    mid: Biquad,
    high: Biquad,
    sample_rate: f32,
    low_gain: f32,
    mid_gain: f32,
    high_gain: f32,
    kill_low: bool,
    kill_mid: bool,
    kill_high: bool,
}

impl ThreeBandEq {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            low: Biquad::identity(),
            mid: Biquad::identity(),
            high: Biquad::identity(),
            sample_rate,
            low_gain: 1.0,
            mid_gain: 1.0,
            high_gain: 1.0,
            kill_low: false,
            kill_mid: false,
            kill_high: false,
        };
        s.set_gains(0.5, 0.5, 0.5);
        s
    }

    pub fn set_kills(&mut self, low: bool, mid: bool, high: bool) {
        self.kill_low = low;
        self.kill_mid = mid;
        self.kill_high = high;
    }

    pub fn set_gains(&mut self, low_ui: f32, mid_ui: f32, high_ui: f32) {
        self.low_gain = if self.kill_low {
            0.0
        } else {
            isolator_band_gain(low_ui)
        };
        self.mid_gain = if self.kill_mid {
            0.0
        } else {
            isolator_band_gain(mid_ui)
        };
        self.high_gain = if self.kill_high {
            0.0
        } else {
            isolator_band_gain(high_ui)
        };

        // Shelves/peak shaped for isolator feel; gains applied as parallel-ish mix.
        // Practical approach: process full-band filters at mild settings then scale bands
        // by reconstructing low/mid/high via complementary shelves.
        let low_db = if self.low_gain < 1e-4 {
            -48.0
        } else {
            20.0 * self.low_gain.log10()
        };
        let mid_db = if self.mid_gain < 1e-4 {
            -48.0
        } else {
            20.0 * self.mid_gain.log10()
        };
        let high_db = if self.high_gain < 1e-4 {
            -48.0
        } else {
            20.0 * self.high_gain.log10()
        };

        self.low = Biquad::low_shelf(self.sample_rate, 250.0, low_db);
        self.mid = Biquad::peaking(self.sample_rate, 1000.0, 0.7, mid_db);
        self.high = Biquad::high_shelf(self.sample_rate, 4000.0, high_db);
    }

    pub fn process_sample(&mut self, x: f32) -> f32 {
        let x = self.low.process(x);
        let x = self.mid.process(x);
        self.high.process(x)
    }
}

/// Quantized loop length in source frames from playhead.
pub fn loop_region_frames(
    playhead_frames: f64,
    track_bpm: f64,
    sample_rate: f32,
    beats: f64,
) -> (f64, f64) {
    let sr = sample_rate.max(1.0) as f64;
    let bpm = track_bpm.max(1.0);
    let beat_frames = sr * 60.0 / bpm;
    let start = (playhead_frames / beat_frames).floor() * beat_frames;
    let end = start + beats * beat_frames;
    (start.max(0.0), end.max(start + 64.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolator_center_unity() {
        let g = isolator_band_gain(0.5);
        assert!((g - 1.0).abs() < 1e-4);
        assert!(isolator_db_label(0.5).abs() < 0.1);
    }

    #[test]
    fn isolator_kill_near_zero() {
        let g = isolator_band_gain(0.0);
        assert!(g < 1e-5);
        assert!(isolator_band_gain(0.05) < 0.02);
    }

    #[test]
    fn isolator_boost_top() {
        let g = isolator_band_gain(1.0);
        assert!((g - 10f32.powf(6.0 / 20.0)).abs() < 1e-3);
    }

    #[test]
    fn flat_process_approx() {
        let mut eq = ThreeBandEq::new(48000.0);
        eq.set_gains(0.5, 0.5, 0.5);
        let y = eq.process_sample(0.5);
        assert!((y - 0.5).abs() < 0.08);
    }

    #[test]
    fn loop_four_and_eight_beats() {
        let sr = 48000.0;
        let bpm = 120.0;
        let playhead = sr as f64 * 2.0; // 2 seconds = 4 beats at 120
        let (s4, e4) = loop_region_frames(playhead, bpm, sr, 4.0);
        let beat = sr as f64 * 60.0 / bpm;
        assert!((e4 - s4 - 4.0 * beat).abs() < 1.0);
        let (s8, e8) = loop_region_frames(playhead, bpm, sr, 8.0);
        assert!((e8 - s8 - 8.0 * beat).abs() < 1.0);
        assert!(s4 <= playhead + 1.0);
    }
}
