//! Combined filter: left = LPF, center = bypass, right = HPF.

#[derive(Clone, Copy)]
struct OnePole {
    a: f32,
    z: f32,
}

impl OnePole {
    fn lowpass(sample_rate: f32, cutoff: f32) -> Self {
        let x = (-2.0 * std::f32::consts::PI * cutoff / sample_rate).exp();
        Self { a: 1.0 - x, z: 0.0 }
    }

    fn process_lp(&mut self, x: f32) -> f32 {
        self.z += self.a * (x - self.z);
        self.z
    }

    fn process_hp(&mut self, x: f32) -> f32 {
        let lp = self.process_lp(x);
        x - lp
    }
}

pub struct DeckFilter {
    sample_rate: f32,
    lp: OnePole,
    hp: OnePole,
    /// 0 = full LPF, 0.5 = bypass, 1 = full HPF
    amount: f32,
}

impl DeckFilter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            lp: OnePole::lowpass(sample_rate, 8000.0),
            hp: OnePole::lowpass(sample_rate, 200.0),
            amount: 0.5,
        }
    }

    pub fn set_amount(&mut self, ui: f32) {
        self.amount = ui.clamp(0.0, 1.0);
        if self.amount < 0.5 {
            let t = self.amount / 0.5;
            let cutoff = 200.0 * (18000.0f32 / 200.0).powf(t);
            self.lp = OnePole::lowpass(self.sample_rate, cutoff);
        } else {
            let t = (self.amount - 0.5) / 0.5;
            let cutoff = 80.0 * (8000.0f32 / 80.0).powf(t);
            self.hp = OnePole::lowpass(self.sample_rate, cutoff);
        }
    }

    pub fn process_sample(&mut self, x: f32) -> f32 {
        if (self.amount - 0.5).abs() < 0.02 {
            return x;
        }
        if self.amount < 0.5 {
            let wet = self.lp.process_lp(x);
            let mix = 1.0 - self.amount / 0.5;
            x * (1.0 - mix) + wet * mix
        } else {
            let wet = self.hp.process_hp(x);
            let mix = (self.amount - 0.5) / 0.5;
            x * (1.0 - mix) + wet * mix
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_near_center() {
        let mut f = DeckFilter::new(48000.0);
        f.set_amount(0.5);
        assert!((f.process_sample(0.25) - 0.25).abs() < 1e-5);
    }
}