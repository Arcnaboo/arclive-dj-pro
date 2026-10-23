//! Three-band EQ using cascaded biquads (low shelf / peak / high shelf).

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

/// UI 0..1 maps to ±12 dB with 0.5 = flat.
pub fn ui_to_db(ui: f32) -> f32 {
    (ui.clamp(0.0, 1.0) - 0.5) * 24.0
}

pub struct ThreeBandEq {
    low: Biquad,
    mid: Biquad,
    high: Biquad,
    sample_rate: f32,
}

impl ThreeBandEq {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            low: Biquad::identity(),
            mid: Biquad::identity(),
            high: Biquad::identity(),
            sample_rate,
        };
        s.set_gains(0.5, 0.5, 0.5);
        s
    }

    pub fn set_gains(&mut self, low_ui: f32, mid_ui: f32, high_ui: f32) {
        self.low = Biquad::low_shelf(self.sample_rate, 220.0, ui_to_db(low_ui));
        self.mid = Biquad::peaking(self.sample_rate, 1000.0, 0.8, ui_to_db(mid_ui));
        self.high = Biquad::high_shelf(self.sample_rate, 5000.0, ui_to_db(high_ui));
    }

    pub fn process_sample(&mut self, x: f32) -> f32 {
        let x = self.low.process(x);
        let x = self.mid.process(x);
        self.high.process(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_unity_approx() {
        let mut eq = ThreeBandEq::new(48000.0);
        eq.set_gains(0.5, 0.5, 0.5);
        let y = eq.process_sample(0.5);
        assert!((y - 0.5).abs() < 0.05);
    }

    #[test]
    fn ui_db_range() {
        assert!((ui_to_db(0.0) + 12.0).abs() < 0.01);
        assert!((ui_to_db(1.0) - 12.0).abs() < 0.01);
        assert!(ui_to_db(0.5).abs() < 0.01);
    }
}