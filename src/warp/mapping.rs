/// Tempo match ratio: playback speed factor (source advances faster when > 1).
pub fn tempo_ratio(track_bpm: f64, master_bpm: f64) -> f64 {
    if track_bpm <= 0.0 || master_bpm <= 0.0 {
        return 1.0;
    }
    master_bpm / track_bpm
}

#[derive(Debug, Clone)]
pub struct ConstantWarp {
    pub track_bpm: f64,
    pub master_bpm: f64,
    pub pitch_preserve: bool,
}

impl ConstantWarp {
    pub fn ratio(&self) -> f64 {
        tempo_ratio(self.track_bpm, self.master_bpm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_examples() {
        assert!((tempo_ratio(138.0, 146.0) - 146.0 / 138.0).abs() < 1e-9);
        assert!((tempo_ratio(150.0, 146.0) - 146.0 / 150.0).abs() < 1e-9);
    }
}