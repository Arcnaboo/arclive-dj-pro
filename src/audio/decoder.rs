use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Clone)]
pub struct TrackMeta {
    pub title: String,
    pub artist: String,
    pub path: String,
    pub format: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_secs: f64,
}

#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub meta: TrackMeta,
    /// Interleaved stereo f32 at original sample rate (mono duplicated).
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

pub fn decode_file(path: &Path) -> Result<DecodedAudio> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("probe audio")?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("no supported audio track"))?
        .clone();

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| anyhow!("missing sample rate"))?;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2) as u16;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("create decoder")?;

    let mut title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let mut artist = String::new();

    if let Some(metadata) = format.metadata().current() {
        for tag in metadata.tags() {
            let key = tag.key.to_ascii_lowercase();
            if key.contains("title") && !tag.value.to_string().is_empty() {
                title = tag.value.to_string();
            }
            if (key.contains("artist") || key.contains("albumartist"))
                && !tag.value.to_string().is_empty()
            {
                artist = tag.value.to_string();
            }
        }
    }

    let mut interleaved = Vec::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(anyhow!("decode packet: {e}")),
        };

        if packet.track_id() != track.id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                if sample_buf.is_none() {
                    let spec = *decoded.spec();
                    let duration = decoded.capacity() as u64;
                    sample_buf = Some(SampleBuffer::new(duration, spec));
                }
                if let Some(buf) = sample_buf.as_mut() {
                    buf.copy_interleaved_ref(decoded);
                    let samples = buf.samples();
                    push_as_stereo(&mut interleaved, samples, channels);
                }
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(anyhow!("decode: {e}")),
        }
    }

    if interleaved.is_empty() {
        return Err(anyhow!("decoded zero samples"));
    }

    let frames = interleaved.len() / 2;
    let duration_secs = frames as f64 / sample_rate as f64;
    let format_str = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("?")
        .to_ascii_uppercase();

    Ok(DecodedAudio {
        meta: TrackMeta {
            title,
            artist,
            path: path.to_string_lossy().to_string(),
            format: format_str,
            sample_rate,
            channels,
            duration_secs,
        },
        samples: interleaved,
        sample_rate,
    })
}

fn push_as_stereo(out: &mut Vec<f32>, samples: &[f32], channels: u16) {
    match channels {
        1 => {
            for &s in samples {
                out.push(s);
                out.push(s);
            }
        }
        2 => out.extend_from_slice(samples),
        n => {
            let n = n as usize;
            for frame in samples.chunks(n) {
                out.push(*frame.first().unwrap_or(&0.0));
                out.push(*frame.get(1).unwrap_or(frame.first().unwrap_or(&0.0)));
            }
        }
    }
}

/// Resample interleaved stereo to `to` Hz so the engine plays it at the device rate.
pub fn resample_stereo(samples: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    use rubato::{FftFixedIn, Resampler};
    if from == to || samples.is_empty() {
        return Ok(samples.to_vec());
    }
    let frames = samples.len() / 2;
    let chunk = 4096;
    let mut rs = FftFixedIn::<f32>::new(from as usize, to as usize, chunk, 2, 2)
        .map_err(|e| anyhow!("resampler: {e}"))?;
    let delay = rs.output_delay();
    let expected = (frames as u64 * to as u64 / from as u64) as usize;

    let mut left = Vec::with_capacity(expected + chunk);
    let mut right = Vec::with_capacity(expected + chunk);
    let mut in_l = vec![0.0f32; chunk];
    let mut in_r = vec![0.0f32; chunk];
    let mut pos = 0usize;
    // Feed zero-padded chunks until the delayed tail has been flushed out.
    while left.len() < expected + delay {
        for i in 0..chunk {
            let f = pos + i;
            let (l, r) = if f < frames {
                (samples[f * 2], samples[f * 2 + 1])
            } else {
                (0.0, 0.0)
            };
            in_l[i] = l;
            in_r[i] = r;
        }
        pos += chunk;
        let out = rs
            .process(&[&in_l[..], &in_r[..]], None)
            .map_err(|e| anyhow!("resample: {e}"))?;
        left.extend_from_slice(&out[0]);
        right.extend_from_slice(&out[1]);
    }

    let mut interleaved = Vec::with_capacity(expected * 2);
    for i in delay..delay + expected {
        interleaved.push(left[i]);
        interleaved.push(right[i]);
    }
    Ok(interleaved)
}

/// Build peak waveform summary for UI (normalized 0..1).
pub fn waveform_peaks(samples: &[f32], buckets: usize) -> Vec<f32> {
    if samples.is_empty() || buckets == 0 {
        return vec![0.0; buckets.max(1)];
    }
    let frames = samples.len() / 2;
    let mut peaks = vec![0.0f32; buckets];
    for (i, peak) in peaks.iter_mut().enumerate() {
        let start = i * frames / buckets;
        let end = ((i + 1) * frames / buckets).max(start + 1);
        let mut m = 0.0f32;
        for f in start..end.min(frames) {
            let l = samples[f * 2].abs();
            let r = samples[f * 2 + 1].abs();
            m = m.max(l).max(r);
        }
        *peak = m;
    }
    let max = peaks.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
    for p in &mut peaks {
        *p /= max;
    }
    peaks
}

pub fn format_duration(secs: f64) -> String {
    let s = secs.max(0.0);
    let m = (s as u64) / 60;
    let rem = s - (m * 60) as f64;
    format!("{m}:{rem:04.1}")
}

#[allow(dead_code)]
fn _silence_check(_buf: AudioBufferRef<'_>) {}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_44k_to_48k_keeps_duration() {
        let frames = 44100;
        let mut input = vec![0.0f32; frames * 2];
        for i in 0..frames {
            let s = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        let out = resample_stereo(&input, 44100, 48000).unwrap();
        assert_eq!(out.len() / 2, 48000);
        let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((peak - 0.5).abs() < 0.05, "peak {peak}");
    }
}
