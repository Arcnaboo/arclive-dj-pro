/// Exponential approach for parameter smoothing in the audio callback.
pub fn smooth_toward(current: f32, target: f32, coeff: f32) -> f32 {
    current + (target - current) * coeff
}

/// Map 0..1 fader to amplitude with a gentle curve (approx fader^1.5).
pub fn fader_to_gain(ui: f32) -> f32 {
    let u = ui.clamp(0.0, 1.0);
    u.powf(1.5)
}

/// Map 0..1 gain knob to linear gain (0..2, center ~1.0 at 0.7).
pub fn gain_knob_to_linear(ui: f32) -> f32 {
    let u = ui.clamp(0.0, 1.0);
    if u <= 0.7 {
        u / 0.7
    } else {
        1.0 + (u - 0.7) / 0.3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fader_ends() {
        assert!((fader_to_gain(0.0)).abs() < 1e-6);
        assert!((fader_to_gain(1.0) - 1.0).abs() < 1e-6);
    }
}