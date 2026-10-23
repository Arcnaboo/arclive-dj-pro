//! Pitch-preserving WSOLA time stretcher for constant-tempo warp.

use std::f32::consts::PI;

pub struct WsolaStretcher {
    channels: usize,
    window: Vec<f32>,
    grain: usize,
    overlap: usize,
    /// Input read position in frames.
    pub read_pos: f64,
    ratio: f64,
}

impl WsolaStretcher {
    pub fn new(channels: usize, grain: usize) -> Self {
        let grain = grain.max(64);
        let overlap = grain / 2;
        let window = hann(grain);
        Self {
            channels,
            window,
            grain,
            overlap,
            read_pos: 0.0,
            ratio: 1.0,
        }
    }

    pub fn set_ratio(&mut self, ratio: f64) {
        self.ratio = ratio.clamp(0.5, 2.0);
    }

    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    pub fn seek_frames(&mut self, frames: f64) {
        self.read_pos = frames.max(0.0);
    }

    /// Produce `out_frames` stereo (or multi-channel interleaved) samples from `input`.
    pub fn process(
        &mut self,
        input: &[f32],
        out_frames: usize,
        out: &mut [f32],
    ) -> usize {
        let ch = self.channels;
        let in_frames = input.len() / ch;
        if in_frames == 0 || out_frames == 0 {
            return 0;
        }

        let hop_out = self.overlap.max(1);
        let hop_in = (hop_out as f64 * self.ratio).max(1.0);

        let mut written = 0usize;
        // Clear output
        for s in out.iter_mut().take(out_frames * ch) {
            *s = 0.0;
        }

        while written + self.grain <= out_frames {
            let mut center = self.read_pos as isize;
            // Local search for best overlap correlation
            let search = 24isize;
            let mut best = center;
            let mut best_score = f32::MIN;
            for delta in -search..=search {
                let c = center + delta;
                if c < 0 || (c as usize) + self.grain >= in_frames {
                    continue;
                }
                let score = self.correlate(input, c as usize, written, out, out_frames);
                if score > best_score {
                    best_score = score;
                    best = c;
                }
            }
            center = best;
            if center < 0 || (center as usize) + self.grain >= in_frames {
                break;
            }

            for i in 0..self.grain {
                let out_i = written + i;
                if out_i >= out_frames {
                    break;
                }
                let w = self.window[i];
                for c in 0..ch {
                    let sample = input[(center as usize + i) * ch + c];
                    out[out_i * ch + c] += sample * w;
                }
            }

            written += hop_out;
            self.read_pos += hop_in;
            if self.read_pos as usize + self.grain >= in_frames {
                break;
            }
        }

        // Normalize overlapping adds roughly
        for i in 0..written * ch {
            out[i] *= 0.7;
        }
        written
    }

    fn correlate(
        &self,
        input: &[f32],
        center: usize,
        out_pos: usize,
        out: &[f32],
        out_frames: usize,
    ) -> f32 {
        let ch = self.channels;
        let n = self.overlap.min(32);
        if out_pos < n || center < n {
            return 0.0;
        }
        let mut score = 0.0f32;
        for i in 0..n {
            let oi = out_pos - n + i;
            let ii = center - n + i;
            if oi >= out_frames || ii * ch >= input.len() {
                break;
            }
            // Compare left channel
            score += out[oi * ch] * input[ii * ch];
        }
        score
    }
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (n as f32 - 1.0)).cos())
        .collect()
}

/// Streaming WSOLA for the audio callback. Works for any callback buffer size by
/// keeping the overlap tail and finished frames between calls.
pub struct RealtimeWsola {
    grain: usize,
    hop: usize,
    window: Vec<f32>,
    ratio: f64,
    /// Nominal source frame for the next grain.
    read_pos: f64,
    /// Source frame of the audio currently leaving the stretcher.
    out_pos: f64,
    /// Start frame of the previous grain (None after a seek).
    prev_start: Option<usize>,
    /// Second half of the previous windowed grain (interleaved stereo, `hop` frames).
    tail: Vec<f32>,
    /// Finished output frames not yet handed to the device.
    fifo: std::collections::VecDeque<f32>,
}

impl RealtimeWsola {
    pub fn new() -> Self {
        let grain = 1024;
        let hop = grain / 2;
        // Periodic Hann sums to exactly 1 at 50% overlap.
        let window = (0..grain)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / grain as f32).cos())
            .collect();
        Self {
            grain,
            hop,
            window,
            ratio: 1.0,
            read_pos: 0.0,
            out_pos: 0.0,
            prev_start: None,
            tail: vec![0.0; hop * 2],
            fifo: std::collections::VecDeque::with_capacity(8192 * 2),
        }
    }

    /// Tempo ratio: > 1 plays faster (source advances `ratio` frames per output frame).
    pub fn set_ratio(&mut self, ratio: f64) {
        self.ratio = ratio.clamp(0.5, 2.0);
    }

    pub fn seek_frames(&mut self, frames: f64) {
        self.read_pos = frames.max(0.0);
        self.out_pos = self.read_pos;
        self.prev_start = None;
        self.tail.iter_mut().for_each(|s| *s = 0.0);
        self.fifo.clear();
    }

    /// Shift the source position without flushing (smooth, for phase correction).
    pub fn nudge_frames(&mut self, delta: f64) {
        self.read_pos = (self.read_pos + delta).max(0.0);
        self.out_pos = (self.out_pos + delta).max(0.0);
    }

    /// Source position of the audio currently leaving the stretcher.
    pub fn read_pos(&self) -> f64 {
        self.out_pos
    }

    /// Fill interleaved stereo `out` from source `input` (interleaved stereo).
    pub fn fill(&mut self, input: &[f32], out: &mut [f32]) {
        let frames = out.len() / 2;
        while self.fifo.len() / 2 < frames {
            self.push_grain(input);
        }
        for s in out.iter_mut() {
            *s = self.fifo.pop_front().unwrap_or(0.0);
        }
        self.out_pos += frames as f64 * self.ratio;
    }

    fn push_grain(&mut self, input: &[f32]) {
        let in_frames = input.len() / 2;
        let nominal = self.read_pos as usize;
        let start = match self.prev_start {
            Some(prev) => self.best_start(input, prev + self.hop, nominal),
            None => nominal,
        };

        for i in 0..self.grain {
            let f = start + i;
            let w = self.window[i];
            let (l, r) = if f < in_frames {
                (input[f * 2] * w, input[f * 2 + 1] * w)
            } else {
                (0.0, 0.0)
            };
            if i < self.hop {
                self.fifo.push_back(self.tail[i * 2] + l);
                self.fifo.push_back(self.tail[i * 2 + 1] + r);
            } else {
                let t = (i - self.hop) * 2;
                self.tail[t] = l;
                self.tail[t + 1] = r;
            }
        }

        self.prev_start = Some(start);
        self.read_pos += self.hop as f64 * self.ratio;
    }

    /// Pick the grain start near `nominal` whose opening best matches the natural
    /// continuation of the previous grain (`target`).
    fn best_start(&self, input: &[f32], target: usize, nominal: usize) -> usize {
        let in_frames = input.len() / 2;
        let n = 128usize;
        let search = 128usize;
        if target + n >= in_frames || nominal + search + n >= in_frames {
            return nominal;
        }
        let lo = nominal.saturating_sub(search);
        let mut best = nominal;
        let mut best_score = f32::MIN;
        for c in (lo..=nominal + search).step_by(2) {
            let mut score = 0.0f32;
            for i in 0..n {
                let a = input[(target + i) * 2] + input[(target + i) * 2 + 1];
                let b = input[(c + i) * 2] + input[(c + i) * 2 + 1];
                score += a * b;
            }
            if score > best_score {
                best_score = score;
                best = c;
            }
        }
        best
    }
}

impl Default for RealtimeWsola {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretches_longer_when_ratio_gt_one() {
        // 1 second of 440Hz at 48k
        let sr = 48000;
        let mut input = vec![0.0f32; sr * 2];
        for i in 0..sr {
            let s = (2.0 * PI * 440.0 * i as f32 / sr as f32).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        let mut st = WsolaStretcher::new(2, 1024);
        st.set_ratio(146.0 / 138.0);
        let out_frames = 4096;
        let mut out = vec![0.0f32; out_frames * 2];
        let n = st.process(&input, out_frames, &mut out);
        assert!(n > 0);
        let energy: f32 = out.iter().map(|x| x * x).sum();
        assert!(energy > 1.0);
    }

    fn sine(frames: usize, sr: usize) -> Vec<f32> {
        let mut input = vec![0.0f32; frames * 2];
        for i in 0..frames {
            let s = (2.0 * PI * 440.0 * i as f32 / sr as f32).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        input
    }

    #[test]
    fn realtime_fills_small_callback_buffers() {
        let input = sine(48000, 48000);
        let mut rt = RealtimeWsola::new();
        // Typical WASAPI period (~10 ms) is smaller than one grain.
        let mut out = vec![0.0f32; 480 * 2];
        let mut energy = 0.0f32;
        for _ in 0..20 {
            rt.fill(&input, &mut out);
            energy += out.iter().map(|x| x * x).sum::<f32>();
        }
        assert!(energy > 100.0, "energy {energy}");
        assert!((rt.read_pos() - 20.0 * 480.0).abs() < 1.0);
    }

    #[test]
    fn realtime_handles_large_callback_buffers() {
        let input = sine(48000, 48000);
        let mut rt = RealtimeWsola::new();
        let mut out = vec![0.0f32; 4096 * 2];
        rt.fill(&input, &mut out);
        assert!(out.iter().map(|x| x * x).sum::<f32>() > 100.0);
    }

    #[test]
    fn realtime_ratio_above_one_advances_faster() {
        let input = sine(48000, 48000);
        let mut rt = RealtimeWsola::new();
        rt.set_ratio(146.0 / 138.0);
        let mut out = vec![0.0f32; 480 * 2];
        for _ in 0..10 {
            rt.fill(&input, &mut out);
        }
        assert!(rt.read_pos() > 4800.0);
    }
}
