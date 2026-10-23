use crate::audio::clock::MasterClock;
use crate::mixer::crossfader::{crossfader_gains, CrossfaderCurve};
use crate::mixer::eq::ThreeBandEq;
use crate::mixer::filter::DeckFilter;
use crate::mixer::gain::{fader_to_gain, gain_knob_to_linear, smooth_toward};
use crate::mixer::meter::PeakMeter;
use crate::mixer::routing::CueRouting;
use crate::warp::mapping::tempo_ratio;
use crate::warp::stretch::RealtimeWsola;
use crate::warp::sync::{phase_error_beats, sync_nudge_samples};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckId {
    A,
    B,
}

pub struct DeckAudioState {
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub track_bpm: f64,
    pub playing: bool,
    pub synced: bool,
    pub loop_enabled: bool,
    pub loop_start: f64,
    pub loop_end: f64,
    pub stretcher: RealtimeWsola,
    pub eq: ThreeBandEq,
    pub filter: DeckFilter,
    pub cue_point: f64,
    pub read_frames: f64,
}

impl DeckAudioState {
    fn empty(sr: f32) -> Self {
        let mut s = Self {
            samples: Arc::new(Vec::new()),
            sample_rate: sr as u32,
            track_bpm: 120.0,
            playing: false,
            synced: false,
            loop_enabled: false,
            loop_start: 0.0,
            loop_end: 0.0,
            stretcher: RealtimeWsola::new(),
            eq: ThreeBandEq::new(sr),
            filter: DeckFilter::new(sr),
            cue_point: 0.0,
            read_frames: 0.0,
        };
        s.stretcher.set_ratio(1.0);
        s
    }
}

#[derive(Clone)]
pub struct SharedMeters {
    pub a: Arc<AtomicU32>,
    pub b: Arc<AtomicU32>,
    pub master: Arc<AtomicU32>,
    pub clip_a: Arc<AtomicBool>,
    pub clip_b: Arc<AtomicBool>,
    pub clip_master: Arc<AtomicBool>,
    pub pos_a: Arc<AtomicU32>,
    pub pos_b: Arc<AtomicU32>,
}

impl SharedMeters {
    pub fn new() -> Self {
        Self {
            a: Arc::new(AtomicU32::new(0)),
            b: Arc::new(AtomicU32::new(0)),
            master: Arc::new(AtomicU32::new(0)),
            clip_a: Arc::new(AtomicBool::new(false)),
            clip_b: Arc::new(AtomicBool::new(false)),
            clip_master: Arc::new(AtomicBool::new(false)),
            pos_a: Arc::new(AtomicU32::new(0)),
            pos_b: Arc::new(AtomicU32::new(0)),
        }
    }

    fn store_level(atom: &AtomicU32, v: f32) {
        atom.store((v.clamp(0.0, 1.0) * 1_000_000.0) as u32, Ordering::Relaxed);
    }

    pub fn level(atom: &AtomicU32) -> f32 {
        atom.load(Ordering::Relaxed) as f32 / 1_000_000.0
    }
}

struct MixerParams {
    high_a: f32,
    mid_a: f32,
    low_a: f32,
    filter_a: f32,
    gain_a: f32,
    fader_a: f32,
    high_b: f32,
    mid_b: f32,
    low_b: f32,
    filter_b: f32,
    gain_b: f32,
    fader_b: f32,
    crossfader: f32,
    curve: CrossfaderCurve,
    cue: CueRouting,
}

impl Default for MixerParams {
    fn default() -> Self {
        Self {
            high_a: 0.5,
            mid_a: 0.5,
            low_a: 0.5,
            filter_a: 0.5,
            gain_a: 0.7,
            fader_a: 1.0,
            high_b: 0.5,
            mid_b: 0.5,
            low_b: 0.5,
            filter_b: 0.5,
            gain_b: 0.7,
            fader_b: 1.0,
            crossfader: 0.5,
            curve: CrossfaderCurve::EqualPower,
            cue: CueRouting::default(),
        }
    }
}

struct EngineInner {
    deck_a: DeckAudioState,
    deck_b: DeckAudioState,
    params: MixerParams,
    master_bpm: f64,
    sample_rate: f32,
    sm_gain_a: f32,
    sm_gain_b: f32,
    meter_a: PeakMeter,
    meter_b: PeakMeter,
    meter_m: PeakMeter,
    clock_running: bool,
}

pub enum EngineCommand {
    LoadDeck {
        id: DeckId,
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        track_bpm: f64,
        master_bpm: f64,
    },
    Play(DeckId),
    Pause(DeckId),
    Cue(DeckId),
    Seek {
        id: DeckId,
        frames: f64,
    },
    SetSync {
        id: DeckId,
        synced: bool,
    },
    SetLoop {
        id: DeckId,
        enabled: bool,
        start: f64,
        end: f64,
    },
    SetMasterBpm(f64),
    SetMixer {
        high_a: f32,
        mid_a: f32,
        low_a: f32,
        filter_a: f32,
        gain_a: f32,
        fader_a: f32,
        high_b: f32,
        mid_b: f32,
        low_b: f32,
        filter_b: f32,
        gain_b: f32,
        fader_b: f32,
        crossfader: f32,
    },
    SetCurve(CrossfaderCurve),
    SetCue {
        cue_a: bool,
        cue_b: bool,
        blend: f32,
        hp_vol: f32,
    },
}

pub struct AudioEngine {
    inner: Arc<Mutex<EngineInner>>,
    meters: SharedMeters,
    clock: Arc<MasterClock>,
    _stream: Stream,
    cmd_tx: crossbeam_channel::Sender<EngineCommand>,
}

// cpal::Stream is !Send on some platforms; we only touch it from the owning thread.
unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

impl AudioEngine {
    pub fn start(master_bpm: f64) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no default output device"))?;
        let supported = device.default_output_config()?;
        let sample_format = supported.sample_format();
        let sample_rate = supported.sample_rate().0 as f32;
        let channels = supported.channels() as usize;
        let config = StreamConfig {
            channels: supported.channels(),
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let meters = SharedMeters::new();
        let clock = Arc::new(MasterClock::new(master_bpm));
        let inner = Arc::new(Mutex::new(EngineInner {
            deck_a: DeckAudioState::empty(sample_rate),
            deck_b: DeckAudioState::empty(sample_rate),
            params: MixerParams::default(),
            master_bpm,
            sample_rate,
            sm_gain_a: 0.0,
            sm_gain_b: 0.0,
            meter_a: PeakMeter::new(),
            meter_b: PeakMeter::new(),
            meter_m: PeakMeter::new(),
            clock_running: false,
        }));

        let (cmd_tx, cmd_rx) = crossbeam_channel::bounded::<EngineCommand>(256);
        let cmd_rx = Arc::new(Mutex::new(cmd_rx));

        let devices: Vec<_> = host.output_devices()?.collect();
        let cue_available = devices.len() > 1;
        {
            let mut g = inner.lock();
            g.params.cue.cue_device_available = cue_available;
        }

        let err_fn = |e| eprintln!("audio stream error: {e}");

        let stream = match sample_format {
            SampleFormat::F32 => {
                let inner_cb = Arc::clone(&inner);
                let meters_cb = meters.clone();
                let clock_cb = Arc::clone(&clock);
                let cmd_rx = Arc::clone(&cmd_rx);
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], _| {
                        {
                            let rx = cmd_rx.lock();
                            while let Ok(cmd) = rx.try_recv() {
                                apply_command(&inner_cb, cmd);
                            }
                        }
                        process_block(&inner_cb, &meters_cb, &clock_cb, data, channels);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let inner_cb = Arc::clone(&inner);
                let meters_cb = meters.clone();
                let clock_cb = Arc::clone(&clock);
                let cmd_rx = Arc::clone(&cmd_rx);
                device.build_output_stream(
                    &config,
                    move |data: &mut [i16], _| {
                        {
                            let rx = cmd_rx.lock();
                            while let Ok(cmd) = rx.try_recv() {
                                apply_command(&inner_cb, cmd);
                            }
                        }
                        let mut tmp = vec![0.0f32; data.len()];
                        process_block(&inner_cb, &meters_cb, &clock_cb, &mut tmp, channels);
                        for (o, s) in data.iter_mut().zip(tmp.iter()) {
                            *o = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            other => return Err(anyhow!("unsupported sample format {other:?}")),
        };
        stream.play()?;

        Ok(Self {
            inner,
            meters,
            clock,
            _stream: stream,
            cmd_tx,
        })
    }

    pub fn meters(&self) -> SharedMeters {
        self.meters.clone()
    }

    pub fn clock(&self) -> Arc<MasterClock> {
        Arc::clone(&self.clock)
    }

    pub fn cue_available(&self) -> bool {
        self.inner.lock().params.cue.cue_device_available
    }

    pub fn send(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    pub fn sample_rate(&self) -> f32 {
        self.inner.lock().sample_rate
    }
}

fn apply_command(inner: &Arc<Mutex<EngineInner>>, cmd: EngineCommand) {
    let mut g = inner.lock();
    match cmd {
        EngineCommand::LoadDeck {
            id,
            samples,
            sample_rate,
            track_bpm,
            master_bpm,
        } => {
            let sr = g.sample_rate;
            let deck = match id {
                DeckId::A => &mut g.deck_a,
                DeckId::B => &mut g.deck_b,
            };
            deck.samples = samples;
            deck.sample_rate = sample_rate;
            deck.track_bpm = track_bpm;
            deck.playing = false;
            deck.synced = false;
            deck.read_frames = 0.0;
            deck.cue_point = 0.0;
            deck.stretcher = RealtimeWsola::new();
            deck.stretcher
                .set_ratio(tempo_ratio(track_bpm, master_bpm));
            deck.eq = ThreeBandEq::new(sr);
            deck.filter = DeckFilter::new(sr);
            g.master_bpm = master_bpm;
        }
        EngineCommand::Play(id) => {
            match id {
                DeckId::A => g.deck_a.playing = true,
                DeckId::B => g.deck_b.playing = true,
            }
            g.clock_running = g.deck_a.playing || g.deck_b.playing;
        }
        EngineCommand::Pause(id) => {
            match id {
                DeckId::A => g.deck_a.playing = false,
                DeckId::B => g.deck_b.playing = false,
            }
            g.clock_running = g.deck_a.playing || g.deck_b.playing;
        }
        EngineCommand::Cue(id) => {
            let deck = match id {
                DeckId::A => &mut g.deck_a,
                DeckId::B => &mut g.deck_b,
            };
            deck.playing = false;
            deck.read_frames = deck.cue_point;
            deck.stretcher.seek_frames(deck.cue_point);
        }
        EngineCommand::Seek { id, frames } => {
            let deck = match id {
                DeckId::A => &mut g.deck_a,
                DeckId::B => &mut g.deck_b,
            };
            deck.read_frames = frames.max(0.0);
            deck.stretcher.seek_frames(deck.read_frames);
        }
        EngineCommand::SetSync { id, synced } => {
            let master_bpm = g.master_bpm;
            let deck = match id {
                DeckId::A => &mut g.deck_a,
                DeckId::B => &mut g.deck_b,
            };
            deck.synced = synced;
            if synced {
                let track_bpm = deck.track_bpm;
                deck.stretcher
                    .set_ratio(tempo_ratio(track_bpm, master_bpm));
            }
        }
        EngineCommand::SetLoop {
            id,
            enabled,
            start,
            end,
        } => {
            let deck = match id {
                DeckId::A => &mut g.deck_a,
                DeckId::B => &mut g.deck_b,
            };
            deck.loop_enabled = enabled;
            deck.loop_start = start;
            deck.loop_end = end.max(start + 64.0);
        }
        EngineCommand::SetMasterBpm(bpm) => {
            g.master_bpm = bpm;
            let ra = tempo_ratio(g.deck_a.track_bpm, bpm);
            let rb = tempo_ratio(g.deck_b.track_bpm, bpm);
            g.deck_a.stretcher.set_ratio(ra);
            g.deck_b.stretcher.set_ratio(rb);
        }
        EngineCommand::SetMixer {
            high_a,
            mid_a,
            low_a,
            filter_a,
            gain_a,
            fader_a,
            high_b,
            mid_b,
            low_b,
            filter_b,
            gain_b,
            fader_b,
            crossfader,
        } => {
            g.params.high_a = high_a;
            g.params.mid_a = mid_a;
            g.params.low_a = low_a;
            g.params.filter_a = filter_a;
            g.params.gain_a = gain_a;
            g.params.fader_a = fader_a;
            g.params.high_b = high_b;
            g.params.mid_b = mid_b;
            g.params.low_b = low_b;
            g.params.filter_b = filter_b;
            g.params.gain_b = gain_b;
            g.params.fader_b = fader_b;
            g.params.crossfader = crossfader;
            g.deck_a.eq.set_gains(low_a, mid_a, high_a);
            g.deck_b.eq.set_gains(low_b, mid_b, high_b);
            g.deck_a.filter.set_amount(filter_a);
            g.deck_b.filter.set_amount(filter_b);
        }
        EngineCommand::SetCurve(c) => g.params.curve = c,
        EngineCommand::SetCue {
            cue_a,
            cue_b,
            blend,
            hp_vol,
        } => {
            g.params.cue.cue_a = cue_a;
            g.params.cue.cue_b = cue_b;
            g.params.cue.cue_master_blend = blend;
            g.params.cue.headphone_volume = hp_vol;
        }
    }
}

fn process_block(
    inner: &Arc<Mutex<EngineInner>>,
    meters: &SharedMeters,
    clock: &MasterClock,
    data: &mut [f32],
    out_channels: usize,
) {
    let frames = data.len() / out_channels.max(1);
    let mut g = inner.lock();
    let sr = g.sample_rate;

    let mut buf_a = vec![0.0f32; frames * 2];
    let mut buf_b = vec![0.0f32; frames * 2];

    render_deck(&mut g.deck_a, &mut buf_a, frames);
    render_deck(&mut g.deck_b, &mut buf_b, frames);

    // Phase sync after rendering (tempo already applied via stretcher ratio).
    let master_beats = clock.beats();
    let master_bpm = g.master_bpm;
    apply_phase_sync(&mut g.deck_a, master_beats, master_bpm, sr);
    apply_phase_sync(&mut g.deck_b, master_beats, master_bpm, sr);

    for i in 0..frames {
        for c in 0..2 {
            let idx = i * 2 + c;
            let mut sa = buf_a[idx];
            let mut sb = buf_b[idx];
            sa = g.deck_a.eq.process_sample(sa);
            sa = g.deck_a.filter.process_sample(sa);
            sb = g.deck_b.eq.process_sample(sb);
            sb = g.deck_b.filter.process_sample(sb);
            buf_a[idx] = sa;
            buf_b[idx] = sb;
        }
    }

    let xf = crossfader_gains(g.params.crossfader, g.params.curve);
    let target_a =
        fader_to_gain(g.params.fader_a) * gain_knob_to_linear(g.params.gain_a) * xf.a;
    let target_b =
        fader_to_gain(g.params.fader_b) * gain_knob_to_linear(g.params.gain_b) * xf.b;

    let mut master_tmp = vec![0.0f32; frames * 2];
    for i in 0..frames {
        g.sm_gain_a = smooth_toward(g.sm_gain_a, target_a, 0.02);
        g.sm_gain_b = smooth_toward(g.sm_gain_b, target_b, 0.02);
        for c in 0..2 {
            let idx = i * 2 + c;
            master_tmp[idx] = buf_a[idx] * g.sm_gain_a + buf_b[idx] * g.sm_gain_b;
        }
    }

    g.meter_a.process(&buf_a);
    g.meter_b.process(&buf_b);
    g.meter_m.process(&master_tmp);
    SharedMeters::store_level(&meters.a, g.meter_a.level());
    SharedMeters::store_level(&meters.b, g.meter_b.level());
    SharedMeters::store_level(&meters.master, g.meter_m.level());
    meters.clip_a.store(g.meter_a.clipped(), Ordering::Relaxed);
    meters.clip_b.store(g.meter_b.clipped(), Ordering::Relaxed);
    meters
        .clip_master
        .store(g.meter_m.clipped(), Ordering::Relaxed);

    let total_a = (g.deck_a.samples.len() / 2).max(1) as f64;
    let total_b = (g.deck_b.samples.len() / 2).max(1) as f64;
    meters.pos_a.store(
        ((g.deck_a.stretcher.read_pos() / total_a).clamp(0.0, 1.0) * 1_000_000.0) as u32,
        Ordering::Relaxed,
    );
    meters.pos_b.store(
        ((g.deck_b.stretcher.read_pos() / total_b).clamp(0.0, 1.0) * 1_000_000.0) as u32,
        Ordering::Relaxed,
    );

    if g.clock_running {
        clock.set_bpm(g.master_bpm);
        clock.advance(frames as u32, sr as u32);
    }

    for i in 0..frames {
        let l = master_tmp[i * 2].clamp(-1.0, 1.0);
        let r = master_tmp[i * 2 + 1].clamp(-1.0, 1.0);
        if out_channels == 1 {
            data[i] = (l + r) * 0.5;
        } else {
            data[i * out_channels] = l;
            data[i * out_channels + 1] = r;
            for c in 2..out_channels {
                data[i * out_channels + c] = 0.0;
            }
        }
    }
}

fn render_deck(deck: &mut DeckAudioState, out: &mut [f32], _frames: usize) {
    if !deck.playing || deck.samples.is_empty() {
        for s in out.iter_mut() {
            *s = 0.0;
        }
        return;
    }
    deck.stretcher.fill(&deck.samples, out);
    deck.read_frames = deck.stretcher.read_pos();

    if deck.loop_enabled && deck.read_frames >= deck.loop_end {
        deck.read_frames = deck.loop_start;
        deck.stretcher.seek_frames(deck.loop_start);
    }

    let total = deck.samples.len() / 2;
    if deck.read_frames as usize >= total.saturating_sub(64) {
        deck.playing = false;
    }
}

/// Align a synced deck's phase to the master musical clock (tempo match is separate).
fn apply_phase_sync(deck: &mut DeckAudioState, master_beats: f64, master_bpm: f64, sample_rate: f32) {
    if !deck.synced || deck.track_bpm <= 0.0 || deck.samples.is_empty() {
        return;
    }
    let pos_secs = deck.stretcher.read_pos() / deck.sample_rate.max(1) as f64;
    let deck_beat = pos_secs * deck.track_bpm / 60.0;
    let err = phase_error_beats(deck_beat, master_beats);
    if err.abs() < 0.02 {
        return;
    }
    let nudge = sync_nudge_samples(err, master_bpm, sample_rate as u32);
    deck.stretcher.nudge_frames(-(nudge as f64));
    deck.read_frames = deck.stretcher.read_pos();
}