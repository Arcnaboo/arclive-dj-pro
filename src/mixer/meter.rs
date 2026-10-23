pub struct PeakMeter {
    peak: f32,
    clip: bool,
    decay: f32,
}

impl PeakMeter {
    pub fn new() -> Self {
        Self {
            peak: 0.0,
            clip: false,
            decay: 0.96,
        }
    }

    pub fn process(&mut self, samples: &[f32]) {
        let mut m = 0.0f32;
        for &s in samples {
            m = m.max(s.abs());
        }
        self.peak = (self.peak * self.decay).max(m);
        if m >= 0.999 {
            self.clip = true;
        } else if self.peak < 0.8 {
            self.clip = false;
        }
    }

    pub fn level(&self) -> f32 {
        self.peak.clamp(0.0, 1.0)
    }

    pub fn clipped(&self) -> bool {
        self.clip
    }
}

impl Default for PeakMeter {
    fn default() -> Self {
        Self::new()
    }
}