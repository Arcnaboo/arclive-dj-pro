/// Spectral flux-like onset envelope from interleaved stereo PCM.
pub fn onset_envelope(samples: &[f32], sample_rate: u32, hop: usize) -> Vec<f32> {
    let hop = hop.max(64);
    let frames = samples.len() / 2;
    if frames < hop * 2 {
        return vec![];
    }

    let mut env = Vec::with_capacity(frames / hop);
    let mut prev_energy = 0.0f32;
    for start in (0..frames.saturating_sub(hop)).step_by(hop) {
        let mut e = 0.0f32;
        for i in start..start + hop {
            let l = samples[i * 2];
            let r = samples[i * 2 + 1];
            e += l * l + r * r;
        }
        e = (e / hop as f32).sqrt();
        let flux = (e - prev_energy).max(0.0);
        env.push(flux);
        prev_energy = e;
    }
    // Light smoothing
    if env.len() >= 3 {
        for i in 1..env.len() - 1 {
            env[i] = (env[i - 1] + env[i] + env[i + 1]) / 3.0;
        }
    }
    let _ = sample_rate;
    env
}