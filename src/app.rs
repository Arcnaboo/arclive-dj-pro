use crate::analysis::bpm::analyze_bpm;
use crate::analysis::beat_grid::BeatGrid;
use crate::audio::decoder::{decode_file, format_duration, resample_stereo, waveform_peaks};
use crate::audio::device::list_output_devices;
use crate::audio::engine::{AudioEngine, DeckId, EngineCommand, SharedMeters};
use crate::drop_target::{
    cursor_client_pos, decide_drop, DropDecision, DropLayout, DropZone, Rect,
};
use crate::library::database::{default_db_path, LibraryDb, TrackRow};
use crate::library::metadata::{file_stamp, track_row_from_meta};
use crate::library::scanner::scan_folder;
use crate::mixer::crossfader::CrossfaderCurve;
use crate::settings::config::{load_config, save_config, AppConfig};
use crate::warp::markers::WarpMap;
use anyhow::Result;
use parking_lot::Mutex;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

slint::include_modules!();

#[derive(Clone)]
struct DeckUiState {
    title: String,
    artist: String,
    bpm: f64,
    duration: f64,
    status: String,
    playing: bool,
    synced: bool,
    can_play: bool,
    peaks: Vec<f32>,
    overview: Vec<f32>,
    path: Option<PathBuf>,
    loop_on: bool,
}

impl Default for DeckUiState {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            bpm: 0.0,
            duration: 0.0,
            status: "Empty".into(),
            playing: false,
            synced: false,
            can_play: false,
            peaks: Vec::new(),
            overview: Vec::new(),
            path: None,
            loop_on: false,
        }
    }
}

struct AppState {
    config: AppConfig,
    layout: DropLayout,
    cursor: (f64, f64),
    hover_paths: Vec<PathBuf>,
    hover_zone: DropZone,
    deck_a: DeckUiState,
    deck_b: DeckUiState,
    cue_a: bool,
    cue_b: bool,
    curve: CrossfaderCurve,
    tap_times: Vec<Instant>,
    last_hwnd: isize,
    pending_load_a: Option<PathBuf>,
    pending_load_b: Option<PathBuf>,
}

pub struct ArcLiveApp {
    ui: AppWindow,
    engine: Arc<AudioEngine>,
    meters: SharedMeters,
    db: Arc<Mutex<LibraryDb>>,
    state: Arc<Mutex<AppState>>,
}

impl ArcLiveApp {
    pub fn new() -> Result<Self> {
        let config = load_config();
        let ui = AppWindow::new()?;
        ui.set_master_bpm(config.master_bpm as f32);
        ui.set_master_bpm_text(format!("{:.2}", config.master_bpm).into());
        ui.set_load_protection(config.load_protection);
        ui.set_library_collapsed(config.library_collapsed);
        ui.set_crossfader_curve(config.crossfader_curve.clone().into());

        let engine = match AudioEngine::start(config.master_bpm) {
            Ok(e) => Arc::new(e),
            Err(e) => {
                eprintln!("Audio engine failed to start: {e:#}");
                // Still show UI; create a dummy? Better to fail soft with banner.
                return Err(e);
            }
        };
        let meters = engine.meters();
        ui.set_cue_available(engine.cue_available());
        ui.set_sample_rate_text(format!("{:.0}", engine.sample_rate()).into());

        if let Ok(devices) = list_output_devices() {
            let names: Vec<SharedString> = devices.iter().map(|d| d.name.clone().into()).collect();
            ui.set_audio_devices(ModelRc::new(VecModel::from(names)));
        }

        let db = Arc::new(Mutex::new(LibraryDb::open(&default_db_path())?));
        refresh_library(&ui, &db, "");

        let curve = match config.crossfader_curve.as_str() {
            "Linear" => CrossfaderCurve::Linear,
            "Sharp Cut" => CrossfaderCurve::SharpCut,
            _ => CrossfaderCurve::EqualPower,
        };

        let state = Arc::new(Mutex::new(AppState {
            config: config.clone(),
            layout: DropLayout::default(),
            cursor: (0.0, 0.0),
            hover_paths: Vec::new(),
            hover_zone: DropZone::None,
            deck_a: DeckUiState::default(),
            deck_b: DeckUiState::default(),
            cue_a: false,
            cue_b: false,
            curve,
            tap_times: Vec::new(),
            last_hwnd: 0,
            pending_load_a: None,
            pending_load_b: None,
        }));

        let app = Self {
            ui,
            engine,
            meters,
            db,
            state,
        };
        app.wire_callbacks();
        app.install_file_drop();
        app.start_ui_timer();
        Ok(app)
    }

    pub fn queue_load_a(&self, path: PathBuf) {
        self.state.lock().pending_load_a = Some(path);
    }

    pub fn queue_load_b(&self, path: PathBuf) {
        self.state.lock().pending_load_b = Some(path);
    }

    pub fn run(self) -> Result<()> {
        {
            let ui_weak = self.ui.as_weak();
            let engine = Arc::clone(&self.engine);
            let state = Arc::clone(&self.state);
            let db = Arc::clone(&self.db);
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(250),
                move || {
                    let (a, b) = {
                        let mut st = state.lock();
                        (st.pending_load_a.take(), st.pending_load_b.take())
                    };
                    if let Some(path) = a {
                        load_path_on_deck(&ui_weak, &engine, &state, &db, DeckId::A, path);
                    }
                    if let Some(path) = b {
                        load_path_on_deck(&ui_weak, &engine, &state, &db, DeckId::B, path);
                    }
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_request_layout_sync();
                    }
                },
            );
            std::mem::forget(timer);
        }

        self.ui.run()?;
        let cfg = self.state.lock().config.clone();
        let _ = save_config(&cfg);
        Ok(())
    }

    fn wire_callbacks(&self) {
        let ui = &self.ui;
        let engine = Arc::clone(&self.engine);
        let state = Arc::clone(&self.state);
        let db = Arc::clone(&self.db);

        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_play_a(move || {
                let mut st = state.lock();
                if !st.deck_a.can_play {
                    return;
                }
                st.deck_a.playing = !st.deck_a.playing;
                let playing = st.deck_a.playing;
                drop(st);
                if playing {
                    engine.send(EngineCommand::Play(DeckId::A));
                } else {
                    engine.send(EngineCommand::Pause(DeckId::A));
                }
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_deck_a_playing(playing);
                    ui.set_deck_a_status(if playing { "Playing".into() } else { "Paused".into() });
                }
            });
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_play_b(move || {
                let mut st = state.lock();
                if !st.deck_b.can_play {
                    return;
                }
                st.deck_b.playing = !st.deck_b.playing;
                let playing = st.deck_b.playing;
                drop(st);
                if playing {
                    engine.send(EngineCommand::Play(DeckId::B));
                } else {
                    engine.send(EngineCommand::Pause(DeckId::B));
                }
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_deck_b_playing(playing);
                    ui.set_deck_b_status(if playing { "Playing".into() } else { "Paused".into() });
                }
            });
        }
        {
            let engine = Arc::clone(&engine);
            ui.on_cue_a_btn(move || engine.send(EngineCommand::Cue(DeckId::A)));
        }
        {
            let engine = Arc::clone(&engine);
            ui.on_cue_b_btn(move || engine.send(EngineCommand::Cue(DeckId::B)));
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_sync_a(move || {
                let mut st = state.lock();
                st.deck_a.synced = !st.deck_a.synced;
                let synced = st.deck_a.synced;
                drop(st);
                engine.send(EngineCommand::SetSync {
                    id: DeckId::A,
                    synced,
                });
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_deck_a_synced(synced);
                }
            });
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_sync_b(move || {
                let mut st = state.lock();
                st.deck_b.synced = !st.deck_b.synced;
                let synced = st.deck_b.synced;
                drop(st);
                engine.send(EngineCommand::SetSync {
                    id: DeckId::B,
                    synced,
                });
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_deck_b_synced(synced);
                }
            });
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_loop_a(move || {
                let mut st = state.lock();
                st.deck_a.loop_on = !st.deck_a.loop_on;
                let on = st.deck_a.loop_on;
                let sr = 48000.0;
                let start = 0.0;
                let end = (st.deck_a.duration * sr).max(sr);
                drop(st);
                engine.send(EngineCommand::SetLoop {
                    id: DeckId::A,
                    enabled: on,
                    start,
                    end: start + end.min(sr * 8.0),
                });
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_status_banner(
                        if on {
                            "Deck A loop on (8 bars approx)".into()
                        } else {
                            "Deck A loop off".into()
                        },
                    );
                }
            });
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_loop_b(move || {
                let mut st = state.lock();
                st.deck_b.loop_on = !st.deck_b.loop_on;
                let on = st.deck_b.loop_on;
                drop(st);
                engine.send(EngineCommand::SetLoop {
                    id: DeckId::B,
                    enabled: on,
                    start: 0.0,
                    end: 48000.0 * 8.0,
                });
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_status_banner(if on { "Deck B loop on".into() } else { "Deck B loop off".into() });
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let db = Arc::clone(&db);
            ui.on_browse_a(move || {
                if let Some(path) = native_open_audio() {
                    load_path_on_deck(
                        &ui_weak,
                        &engine,
                        &state,
                        &db,
                        DeckId::A,
                        path,
                    );
                }
            });
        }
        {
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let db = Arc::clone(&db);
            ui.on_browse_b(move || {
                if let Some(path) = native_open_audio() {
                    load_path_on_deck(
                        &ui_weak,
                        &engine,
                        &state,
                        &db,
                        DeckId::B,
                        path,
                    );
                }
            });
        }

        {
            let engine = Arc::clone(&engine);
            let ui_weak = ui.as_weak();
            ui.on_mixer_changed(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    engine.send(EngineCommand::SetMixer {
                        high_a: ui.get_high_a(),
                        mid_a: ui.get_mid_a(),
                        low_a: ui.get_low_a(),
                        filter_a: ui.get_filter_a(),
                        gain_a: ui.get_gain_a(),
                        fader_a: ui.get_fader_a(),
                        high_b: ui.get_high_b(),
                        mid_b: ui.get_mid_b(),
                        low_b: ui.get_low_b(),
                        filter_b: ui.get_filter_b(),
                        gain_b: ui.get_gain_b(),
                        fader_b: ui.get_fader_b(),
                        crossfader: ui.get_crossfader(),
                    });
                    engine.send(EngineCommand::SetCue {
                        cue_a: ui.get_cue_a(),
                        cue_b: ui.get_cue_b(),
                        blend: ui.get_cue_blend(),
                        hp_vol: ui.get_headphone_vol(),
                    });
                }
            });
        }

        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            ui.on_cue_a_toggle(move || {
                let mut st = state.lock();
                st.cue_a = !st.cue_a;
                let v = st.cue_a;
                let b = st.cue_b;
                drop(st);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_cue_a(v);
                    engine.send(EngineCommand::SetCue {
                        cue_a: v,
                        cue_b: b,
                        blend: ui.get_cue_blend(),
                        hp_vol: ui.get_headphone_vol(),
                    });
                    if !ui.get_cue_available() {
                        ui.set_status_banner(
                            "No separate cue output — cue is not mixed into master.".into(),
                        );
                    }
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            ui.on_cue_b_toggle(move || {
                let mut st = state.lock();
                st.cue_b = !st.cue_b;
                let v = st.cue_b;
                let a = st.cue_a;
                drop(st);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_cue_b(v);
                    engine.send(EngineCommand::SetCue {
                        cue_a: a,
                        cue_b: v,
                        blend: ui.get_cue_blend(),
                        hp_vol: ui.get_headphone_vol(),
                    });
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            ui.on_open_settings(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_settings_open(true);
                }
            });
        }
        {
            let ui_weak = ui.as_weak();
            ui.on_close_settings(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_settings_open(false);
                }
            });
        }

        {
            let db = Arc::clone(&db);
            let ui_weak = ui.as_weak();
            ui.on_library_search_changed(move |q| {
                if let Some(ui) = ui_weak.upgrade() {
                    refresh_library(&ui, &db, q.as_str());
                }
            });
        }
        {
            let db = Arc::clone(&db);
            let ui_weak = ui.as_weak();
            ui.on_import_folder(move || {
                if let Some(folder) = native_open_folder() {
                    let paths = scan_folder(&folder);
                    import_paths(&ui_weak, &db, &paths);
                }
            });
        }
        {
            let db = Arc::clone(&db);
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            ui.on_library_row_activated(move |idx| {
                let ui = match ui_weak.upgrade() {
                    Some(u) => u,
                    None => return,
                };
                let tracks = ui.get_library_tracks();
                if idx < 0 || idx as usize >= tracks.row_count() {
                    return;
                }
                let row = tracks.row_data(idx as usize).unwrap();
                let id = row.id.to_string();
                let path = {
                    let db = db.lock();
                    db.get_by_id(&id).ok().flatten().map(|t| PathBuf::from(t.path))
                };
                if let Some(path) = path {
                    // Load to deck A by default on double semantics; single click selects
                    ui.set_library_selected(idx);
                    load_path_on_deck(&ui_weak, &engine, &state, &db, DeckId::A, path);
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_toggle_library(move || {
                let mut st = state.lock();
                st.config.library_collapsed = !st.config.library_collapsed;
                let c = st.config.library_collapsed;
                drop(st);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_library_collapsed(c);
                }
            });
        }

        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_bpm_nudge(move |delta| {
                let mut st = state.lock();
                st.config.master_bpm = (st.config.master_bpm + delta as f64).clamp(40.0, 300.0);
                let bpm = st.config.master_bpm;
                drop(st);
                engine.send(EngineCommand::SetMasterBpm(bpm));
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_master_bpm(bpm as f32);
                    ui.set_master_bpm_text(format!("{bpm:.2}").into());
                }
            });
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_bpm_tap(move || {
                let mut st = state.lock();
                let now = Instant::now();
                st.tap_times.push(now);
                st.tap_times
                    .retain(|t| now.duration_since(*t) < Duration::from_secs(5));
                if st.tap_times.len() >= 2 {
                    let mut intervals = Vec::new();
                    for w in st.tap_times.windows(2) {
                        intervals.push(w[1].duration_since(w[0]).as_secs_f64());
                    }
                    let avg = intervals.iter().sum::<f64>() / intervals.len() as f64;
                    if avg > 0.15 && avg < 2.0 {
                        st.config.master_bpm = (60.0 / avg).clamp(40.0, 300.0);
                    }
                }
                let bpm = st.config.master_bpm;
                drop(st);
                engine.send(EngineCommand::SetMasterBpm(bpm));
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_master_bpm(bpm as f32);
                    ui.set_master_bpm_text(format!("{bpm:.2}").into());
                }
            });
        }

        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_toggle_load_protection(move || {
                let mut st = state.lock();
                st.config.load_protection = !st.config.load_protection;
                let v = st.config.load_protection;
                drop(st);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_load_protection(v);
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let engine = Arc::clone(&engine);
            let ui_weak = ui.as_weak();
            ui.on_cycle_crossfader_curve(move || {
                let mut st = state.lock();
                st.curve = st.curve.next();
                let label = st.curve.label().to_string();
                st.config.crossfader_curve = label.clone();
                let c = st.curve;
                drop(st);
                engine.send(EngineCommand::SetCurve(c));
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_crossfader_curve(label.into());
                }
            });
        }

        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_request_layout_sync(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    // Approximate layout from window size (matches padding/spacing in app.slint).
                    let size = ui.window().size();
                    let scale = ui.window().scale_factor() as f64;
                    let w = size.width as f64 / scale;
                    let h = size.height as f64 / scale;
                    let pad = 12.0;
                    let gap = 10.0;
                    let toolbar = 40.0;
                    let mixer = 180.0;
                    let lib_h = if ui.get_library_collapsed() {
                        36.0
                    } else {
                        (h * 0.22).clamp(160.0, 280.0)
                    };
                    let content_top = pad + toolbar + gap;
                    let lib_y = h - pad - lib_h;
                    let mixer_y = lib_y - gap - mixer;
                    let deck_h = (mixer_y - gap - content_top).max(120.0);
                    let deck_w = (w - pad * 2.0 - gap) / 2.0;
                    let mut st = state.lock();
                    st.layout.deck_a = Rect {
                        x: pad,
                        y: content_top,
                        w: deck_w,
                        h: deck_h,
                    };
                    st.layout.deck_b = Rect {
                        x: pad + deck_w + gap,
                        y: content_top,
                        w: deck_w,
                        h: deck_h,
                    };
                    st.layout.library = Rect {
                        x: pad,
                        y: lib_y,
                        w: w - pad * 2.0,
                        h: lib_h,
                    };
                    ui.set_deck_a_x(st.layout.deck_a.x as f32);
                    ui.set_deck_a_y(st.layout.deck_a.y as f32);
                    ui.set_deck_a_w(st.layout.deck_a.w as f32);
                    ui.set_deck_a_h(st.layout.deck_a.h as f32);
                    ui.set_deck_b_x(st.layout.deck_b.x as f32);
                    ui.set_deck_b_y(st.layout.deck_b.y as f32);
                    ui.set_deck_b_w(st.layout.deck_b.w as f32);
                    ui.set_deck_b_h(st.layout.deck_b.h as f32);
                    ui.set_library_x(st.layout.library.x as f32);
                    ui.set_library_y(st.layout.library.y as f32);
                    ui.set_library_w(st.layout.library.w as f32);
                    ui.set_library_h(st.layout.library.h as f32);
                }
            });
        }
    }

    fn install_file_drop(&self) {
        use slint::winit_030::{EventResult, WinitWindowAccessor};

        let ui_weak = self.ui.as_weak();
        let state = Arc::clone(&self.state);
        let engine = Arc::clone(&self.engine);
        let db = Arc::clone(&self.db);

        self.ui.window().on_winit_window_event(move |_win, event| {
            use slint::winit_030::winit::event::WindowEvent;
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let mut st = state.lock();
                    st.cursor = (position.x, position.y);
                    EventResult::Propagate
                }
                WindowEvent::HoveredFile(path) => {
                    let mut st = state.lock();
                    if !st.hover_paths.contains(path) {
                        st.hover_paths.push(path.clone());
                    }
                    // Refresh cursor from Win32 for accurate hit-test during OS drag.
                    if let Some((x, y)) = cursor_client_pos(st.last_hwnd) {
                        st.cursor = (x, y);
                    }
                    let zone = st.layout.hit_test(st.cursor.0, st.cursor.1);
                    st.hover_zone = zone;
                    drop(st);
                    update_drop_hover(&ui_weak, zone);
                    EventResult::PreventDefault
                }
                WindowEvent::HoveredFileCancelled => {
                    {
                        let mut st = state.lock();
                        st.hover_paths.clear();
                        st.hover_zone = DropZone::None;
                    }
                    update_drop_hover(&ui_weak, DropZone::None);
                    EventResult::PreventDefault
                }
                WindowEvent::DroppedFile(path) => {
                    let (zone, paths, playing, protection) = {
                        let mut st = state.lock();
                        if let Some((x, y)) = cursor_client_pos(st.last_hwnd) {
                            st.cursor = (x, y);
                        }
                        if !st.hover_paths.iter().any(|p| p == path) {
                            st.hover_paths.push(path.clone());
                        }
                        let zone = st.layout.hit_test(st.cursor.0, st.cursor.1);
                        let paths = st.hover_paths.clone();
                        st.hover_paths.clear();
                        st.hover_zone = DropZone::None;
                        let playing = match zone {
                            DropZone::DeckA => st.deck_a.playing,
                            DropZone::DeckB => st.deck_b.playing,
                            _ => false,
                        };
                        let protection = st.config.load_protection;
                        (zone, paths, playing, protection)
                    };
                    update_drop_hover(&ui_weak, DropZone::None);
                    handle_drop_decision(
                        &ui_weak,
                        &engine,
                        &state,
                        &db,
                        decide_drop(zone, &paths, playing, protection),
                    );
                    EventResult::PreventDefault
                }
                _ => EventResult::Propagate,
            }
        });

        // Capture HWND for ScreenToClient
        #[cfg(windows)]
        {
            let state = Arc::clone(&self.state);
            let ui_weak = self.ui.as_weak();
            slint::Timer::default().start(
                slint::TimerMode::Repeated,
                Duration::from_millis(500),
                move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.window().with_winit_window(|winit_window| {
                            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                            if let Ok(handle) = winit_window.window_handle() {
                                if let RawWindowHandle::Win32(h) = handle.as_raw() {
                                    state.lock().last_hwnd = h.hwnd.get() as isize;
                                }
                            }
                        });
                    }
                },
            );
        }
    }

    fn start_ui_timer(&self) {
        let ui_weak = self.ui.as_weak();
        let meters = self.meters.clone();
        let state = Arc::clone(&self.state);

        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(33),
            move || {
                let Some(ui) = ui_weak.upgrade() else {
                    return;
                };
                ui.set_meter_a(SharedMeters::level(&meters.a));
                ui.set_meter_b(SharedMeters::level(&meters.b));
                ui.set_meter_master(SharedMeters::level(&meters.master));
                ui.set_clip_a(meters.clip_a.load(Ordering::Relaxed));
                ui.set_clip_b(meters.clip_b.load(Ordering::Relaxed));
                ui.set_clip_master(meters.clip_master.load(Ordering::Relaxed));
                let pa = meters.pos_a.load(Ordering::Relaxed) as f32 / 1_000_000.0;
                let pb = meters.pos_b.load(Ordering::Relaxed) as f32 / 1_000_000.0;
                ui.set_deck_a_playhead(pa);
                ui.set_deck_b_playhead(pb);

                let st = state.lock();
                if st.deck_a.duration > 0.0 {
                    ui.set_deck_a_time(format_duration(pa as f64 * st.deck_a.duration).into());
                }
                if st.deck_b.duration > 0.0 {
                    ui.set_deck_b_time(format_duration(pb as f64 * st.deck_b.duration).into());
                }
            },
        );
        // Keep timer alive by leaking (slint timer needs ownership) — store on UI via forget
        std::mem::forget(timer);
    }
}

fn update_drop_hover(ui_weak: &Weak<AppWindow>, zone: DropZone) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };
    ui.set_deck_a_drop_hover(zone == DropZone::DeckA);
    ui.set_deck_b_drop_hover(zone == DropZone::DeckB);
    ui.set_library_drop_hover(zone == DropZone::Library);
    ui.set_deck_a_drop_message(
        if zone == DropZone::DeckA {
            "Drop to load Deck A".into()
        } else {
            "".into()
        },
    );
    ui.set_deck_b_drop_message(
        if zone == DropZone::DeckB {
            "Drop to load Deck B".into()
        } else {
            "".into()
        },
    );
    ui.set_library_drop_message(
        if zone == DropZone::Library {
            "Drop to import into library".into()
        } else {
            "".into()
        },
    );
}

fn handle_drop_decision(
    ui_weak: &Weak<AppWindow>,
    engine: &Arc<AudioEngine>,
    state: &Arc<Mutex<AppState>>,
    db: &Arc<Mutex<LibraryDb>>,
    decision: DropDecision,
) {
    match decision {
        DropDecision::Reject { message } => {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_status_banner(message.into());
            }
        }
        DropDecision::ImportLibrary { paths } => {
            import_paths(ui_weak, db, &paths);
        }
        DropDecision::LoadDeck { zone, path } => {
            let id = match zone {
                DropZone::DeckA => DeckId::A,
                DropZone::DeckB => DeckId::B,
                _ => return,
            };
            load_path_on_deck(ui_weak, engine, state, db, id, path);
        }
    }
}

fn import_paths(ui_weak: &Weak<AppWindow>, db: &Arc<Mutex<LibraryDb>>, paths: &[PathBuf]) {
    let ui_weak = ui_weak.clone();
    let db = Arc::clone(db);
    let paths = paths.to_vec();
    thread::spawn(move || {
        let mut imported = 0usize;
        for path in &paths {
            match decode_file(path) {
                Ok(decoded) => {
                    let mut row = track_row_from_meta(&decoded.meta, "Imported");
                    // Quick BPM for library
                    let analysis = analyze_bpm(&decoded.samples, decoded.sample_rate);
                    row.bpm = analysis.bpm;
                    row.status = "Analyzed".into();
                    if db.lock().upsert_track(&row).is_ok() {
                        imported += 1;
                    }
                }
                Err(e) => eprintln!("import failed {}: {e:#}", path.display()),
            }
        }
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                refresh_library(&ui, &db, ui.get_library_search().as_str());
                ui.set_status_banner(format!("Imported {imported} track(s).").into());
            }
        });
    });
}

fn load_path_on_deck(
    ui_weak: &Weak<AppWindow>,
    engine: &Arc<AudioEngine>,
    state: &Arc<Mutex<AppState>>,
    db: &Arc<Mutex<LibraryDb>>,
    id: DeckId,
    path: PathBuf,
) {
    {
        let st = state.lock();
        let playing = match id {
            DeckId::A => st.deck_a.playing,
            DeckId::B => st.deck_b.playing,
        };
        if playing && st.config.load_protection {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_status_banner(
                    "Load protection: stop the deck or disable protection in Settings.".into(),
                );
            }
            return;
        }
    }

    if let Some(ui) = ui_weak.upgrade() {
        match id {
            DeckId::A => {
                ui.set_deck_a_status("Loading".into());
                ui.set_deck_a_can_play(false);
            }
            DeckId::B => {
                ui.set_deck_b_status("Loading".into());
                ui.set_deck_b_can_play(false);
            }
        }
        ui.set_status_banner(format!("Loading {}…", path.display()).into());
    }

    let ui_weak = ui_weak.clone();
    let engine = Arc::clone(engine);
    let state = Arc::clone(state);
    let db = Arc::clone(db);

    thread::spawn(move || {
        let set_status = |status: &str| {
            let status = status.to_string();
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    match id {
                        DeckId::A => ui.set_deck_a_status(status.clone().into()),
                        DeckId::B => ui.set_deck_b_status(status.into()),
                    }
                }
            });
        };

        set_status("Loading");
        let decoded = match decode_file(&path) {
            Ok(d) => d,
            Err(e) => {
                let msg = format!("Load error: {e:#}");
                let ui_weak = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_status_banner(msg.into());
                        match id {
                            DeckId::A => ui.set_deck_a_status("Error".into()),
                            DeckId::B => ui.set_deck_b_status("Error".into()),
                        }
                    }
                });
                return;
            }
        };

        set_status("Analyzing");
        let (size, mtime) = file_stamp(&path);
        let cached = db
            .lock()
            .get_cached_analysis(&decoded.meta.path, size, mtime)
            .ok()
            .flatten();

        let (bpm, confidence, offset, warp_json) = if let Some(c) = cached {
            c
        } else {
            let a = analyze_bpm(&decoded.samples, decoded.sample_rate);
            let grid = BeatGrid::from_analysis(&a);
            let mut warp = WarpMap::from_constant(grid.effective_bpm(), decoded.meta.duration_secs);
            // Seed an editable mid-track marker for elastic warp corrections.
            let mid_beat = grid.effective_bpm() * decoded.meta.duration_secs / 120.0;
            let mid_src = warp.beat_to_source(mid_beat);
            warp.add_marker(mid_src, mid_beat);
            let warp_json = serde_json::to_string(&warp).unwrap_or_else(|_| "{}".into());
            let _ = db.lock().cache_analysis(
                &decoded.meta.path,
                size,
                mtime,
                a.bpm,
                a.confidence,
                a.offset_secs,
                &warp_json,
            );
            (a.bpm, a.confidence, a.offset_secs, warp_json)
        };
        let _ = (confidence, offset, warp_json);

        set_status("Preparing warp");
        let peaks = waveform_peaks(&decoded.samples, 256);
        let overview = waveform_peaks(&decoded.samples, 128);
        let master_bpm = state.lock().config.master_bpm;

        let mut row = track_row_from_meta(&decoded.meta, "Ready");
        row.bpm = bpm;
        {
            let db = db.lock();
            let _ = db.upsert_track(&row);
            // Ensure default playlist + hot cue slot 0 at track start exist for workflow.
            if let Ok(pid) = db.create_playlist("Default") {
                let _ = db.add_to_playlist(&pid, &row.id, 0);
            } else {
                // Playlist may already exist — ignore unique errors.
            }
            let _ = db.set_hot_cue(&row.id, 0, 0.0, "Cue 1");
        }

        // The engine renders at the device rate; convert so pitch and tempo are correct.
        let device_rate = engine.sample_rate() as u32;
        let samples = match resample_stereo(&decoded.samples, decoded.sample_rate, device_rate) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("resample failed, playing at source rate: {e:#}");
                decoded.samples.clone()
            }
        };
        let samples = Arc::new(samples);
        engine.send(EngineCommand::LoadDeck {
            id,
            samples,
            sample_rate: device_rate,
            track_bpm: bpm,
            master_bpm,
        });

        let meta = decoded.meta;
        let ui_weak2 = ui_weak.clone();
        let state2 = Arc::clone(&state);
        let db2 = Arc::clone(&db);
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak2.upgrade() else {
                return;
            };
            let peaks_model = ModelRc::new(VecModel::from(peaks));
            let overview_model = ModelRc::new(VecModel::from(overview));
            let bpm_text = format!("{bpm:.2}");
            let eff = format!("→ {master_bpm:.2} BPM");
            let dur = format_duration(meta.duration_secs);

            {
                let mut st = state2.lock();
                let deck = match id {
                    DeckId::A => &mut st.deck_a,
                    DeckId::B => &mut st.deck_b,
                };
                deck.title = meta.title.clone();
                deck.artist = if meta.artist.is_empty() {
                    "Unknown Artist".into()
                } else {
                    meta.artist.clone()
                };
                deck.bpm = bpm;
                deck.duration = meta.duration_secs;
                deck.status = "Ready".into();
                deck.can_play = true;
                deck.playing = false;
                deck.synced = false;
                deck.path = Some(path.clone());
            }

            match id {
                DeckId::A => {
                    ui.set_deck_a_title(meta.title.clone().into());
                    ui.set_deck_a_artist(
                        if meta.artist.is_empty() {
                            "Unknown Artist".into()
                        } else {
                            meta.artist.clone().into()
                        },
                    );
                    ui.set_deck_a_bpm(bpm_text.into());
                    ui.set_deck_a_duration(dur.into());
                    ui.set_deck_a_status("Ready".into());
                    ui.set_deck_a_can_play(true);
                    ui.set_deck_a_playing(false);
                    ui.set_deck_a_synced(false);
                    ui.set_deck_a_peaks(peaks_model);
                    ui.set_deck_a_overview(overview_model);
                    ui.set_deck_a_effective_bpm(eff.into());
                    ui.set_deck_a_playhead(0.0);
                }
                DeckId::B => {
                    ui.set_deck_b_title(meta.title.clone().into());
                    ui.set_deck_b_artist(
                        if meta.artist.is_empty() {
                            "Unknown Artist".into()
                        } else {
                            meta.artist.clone().into()
                        },
                    );
                    ui.set_deck_b_bpm(bpm_text.into());
                    ui.set_deck_b_duration(dur.into());
                    ui.set_deck_b_status("Ready".into());
                    ui.set_deck_b_can_play(true);
                    ui.set_deck_b_playing(false);
                    ui.set_deck_b_synced(false);
                    ui.set_deck_b_peaks(peaks_model);
                    ui.set_deck_b_overview(overview_model);
                    ui.set_deck_b_effective_bpm(eff.into());
                    ui.set_deck_b_playhead(0.0);
                }
            }
            refresh_library(&ui, &db2, ui.get_library_search().as_str());
            ui.set_status_banner(
                format!(
                    "Ready: {} ({bpm:.2} BPM, confidence cached/analyzed)",
                    meta.title
                )
                .into(),
            );
        });
    });
}

fn refresh_library(ui: &AppWindow, db: &Arc<Mutex<LibraryDb>>, query: &str) {
    let rows = db.lock().search(query).unwrap_or_default();
    let items: Vec<LibraryTrack> = rows
        .into_iter()
        .map(|t| LibraryTrack {
            id: t.id.into(),
            title: t.title.into(),
            artist: t.artist.into(),
            bpm: if t.bpm > 0.0 {
                format!("{:.2}", t.bpm).into()
            } else {
                "—".into()
            },
            duration: format_duration(t.duration_secs).into(),
            key: t.key.into(),
            format: t.format.into(),
            status: t.status.into(),
        })
        .collect();
    ui.set_library_tracks(ModelRc::new(VecModel::from(items)));
}

fn native_open_audio() -> Option<PathBuf> {
    // Minimal Windows dialog via PowerShell to avoid extra deps.
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object System.Windows.Forms.OpenFileDialog
$f.Filter = 'Audio|*.wav;*.aiff;*.aif;*.flac;*.mp3|All|*.*'
if ($f.ShowDialog() -eq 'OK') { $f.FileName }
"#;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn native_open_folder() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object System.Windows.Forms.FolderBrowserDialog
if ($f.ShowDialog() -eq 'OK') { $f.SelectedPath }
"#;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[allow(dead_code)]
fn _path_exists(p: &Path) -> bool {
    p.exists()
}

#[allow(dead_code)]
fn _track_row_unused(_: &TrackRow) {}
