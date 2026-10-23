use crate::analysis::bpm::analyze_bpm;
use crate::analysis::beat_grid::BeatGrid;
use crate::audio::decoder::{decode_file, format_duration, resample_stereo, waveform_peaks};
use crate::audio::device::list_output_devices;
use crate::audio::engine::{AudioEngine, DeckId, EngineCommand, SharedMeters};
use crate::drop_target::{
    cursor_client_pos, decide_drop, physical_to_logical, DropAccumulator, DropDecision, DropLayout,
    DropZone, Rect,
};
use crate::library::database::{default_db_path, LibraryDb, TrackRow};
use crate::library::metadata::{file_stamp, track_row_from_meta};
use crate::library::scanner::scan_folder;
use crate::mixer::crossfader::CrossfaderCurve;
use crate::mixer::eq::loop_region_frames;
use crate::settings::config::{load_config, save_config, AppConfig};
use crate::warp::markers::WarpMap;
use crate::win_drop::{OleDropEvent, SharedDropLayout};
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
    track_id: Option<String>,
    loop_on: bool,
    loop_start_norm: f32,
    loop_end_norm: f32,
    grid_offset: f64,
    hot_cues: [bool; 8],
    hot_cue_secs: [f64; 8],
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
            track_id: None,
            loop_on: false,
            loop_start_norm: -1.0,
            loop_end_norm: -1.0,
            grid_offset: 0.0,
            hot_cues: [false; 8],
            hot_cue_secs: [0.0; 8],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CtxKind {
    None,
    Library,
    WaveA,
    WaveB,
    DeckA,
    DeckB,
}

struct AppState {
    config: AppConfig,
    layout: DropLayout,
    shared_layout: SharedDropLayout,
    cursor: (f64, f64),
    scale: f64,
    hover_paths: Vec<PathBuf>,
    hover_zone: DropZone,
    drop_acc: DropAccumulator,
    deck_a: DeckUiState,
    deck_b: DeckUiState,
    cue_a: bool,
    cue_b: bool,
    preview_in_master: bool,
    kill_high_a: bool,
    kill_mid_a: bool,
    kill_low_a: bool,
    kill_high_b: bool,
    kill_mid_b: bool,
    kill_low_b: bool,
    curve: CrossfaderCurve,
    tap_times: Vec<Instant>,
    last_hwnd: isize,
    ole_active: bool,
    pending_load_a: Option<PathBuf>,
    pending_load_b: Option<PathBuf>,
    ctx_kind: CtxKind,
    ctx_index: i32,
    ctx_wave_pos: f32,
    drag_track_id: Option<String>,
    drag_label: String,
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
        ui.set_deck_a_hot_cues(ModelRc::new(VecModel::from(vec![false; 8])));
        ui.set_deck_b_hot_cues(ModelRc::new(VecModel::from(vec![false; 8])));

        let engine = match AudioEngine::start(config.master_bpm) {
            Ok(e) => Arc::new(e),
            Err(e) => {
                eprintln!("Audio engine failed to start: {e:#}");
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

        let shared_layout = SharedDropLayout::new();
        let state = Arc::new(Mutex::new(AppState {
            config: config.clone(),
            layout: DropLayout::default(),
            shared_layout: shared_layout.clone(),
            cursor: (0.0, 0.0),
            scale: 1.0,
            hover_paths: Vec::new(),
            hover_zone: DropZone::None,
            drop_acc: DropAccumulator::default(),
            deck_a: DeckUiState::default(),
            deck_b: DeckUiState::default(),
            cue_a: false,
            cue_b: false,
            preview_in_master: false,
            kill_high_a: false,
            kill_mid_a: false,
            kill_low_a: false,
            kill_high_b: false,
            kill_mid_b: false,
            kill_low_b: false,
            curve,
            tap_times: Vec::new(),
            last_hwnd: 0,
            ole_active: false,
            pending_load_a: None,
            pending_load_b: None,
            ctx_kind: CtxKind::None,
            ctx_index: -1,
            ctx_wave_pos: 0.0,
            drag_track_id: None,
            drag_label: String::new(),
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
                },
            );
            std::mem::forget(timer);
        }

        self.ui.run()?;
        crate::win_drop::revoke_if_registered();
        let cfg = self.state.lock().config.clone();
        let _ = save_config(&cfg);
        Ok(())
    }

    fn publish_layout(state: &Arc<Mutex<AppState>>, ui: &AppWindow) {
        let mut st = state.lock();
        st.layout.deck_a = Rect {
            x: ui.get_deck_a_x() as f64,
            y: ui.get_deck_a_y() as f64,
            w: ui.get_deck_a_w() as f64,
            h: ui.get_deck_a_h() as f64,
        };
        st.layout.deck_b = Rect {
            x: ui.get_deck_b_x() as f64,
            y: ui.get_deck_b_y() as f64,
            w: ui.get_deck_b_w() as f64,
            h: ui.get_deck_b_h() as f64,
        };
        st.layout.library = Rect {
            x: ui.get_library_x() as f64,
            y: ui.get_library_y() as f64,
            w: ui.get_library_w() as f64,
            h: ui.get_library_h() as f64,
        };
        st.shared_layout.set_layout(st.layout.clone());
        let scale = ui.window().scale_factor() as f64;
        st.scale = scale;
        st.shared_layout.set_scale(scale);
    }

    fn send_mixer(engine: &AudioEngine, ui: &AppWindow, state: &AppState) {
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
            kill_low_a: state.kill_low_a,
            kill_mid_a: state.kill_mid_a,
            kill_high_a: state.kill_high_a,
            kill_low_b: state.kill_low_b,
            kill_mid_b: state.kill_mid_b,
            kill_high_b: state.kill_high_b,
        });
        engine.send(EngineCommand::SetCue {
            cue_a: state.cue_a,
            cue_b: state.cue_b,
            blend: ui.get_cue_blend(),
            hp_vol: ui.get_headphone_vol(),
            preview_in_master: state.preview_in_master,
        });
    }

    fn open_ctx(ui: &AppWindow, state: &mut AppState, kind: CtxKind, x: f32, y: f32, items: Vec<&str>) {
        state.ctx_kind = kind;
        let model: Vec<SharedString> = items.into_iter().map(|s| s.into()).collect();
        ui.set_ctx_items(ModelRc::new(VecModel::from(model)));
        ui.set_ctx_x(x);
        ui.set_ctx_y(y);
        ui.set_ctx_open(true);
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
            ui.on_play_a(move || toggle_play(&engine, &state, &ui_weak, DeckId::A));
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_play_b(move || toggle_play(&engine, &state, &ui_weak, DeckId::B));
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
            ui.on_sync_a(move || do_sync(&engine, &state, &ui_weak, DeckId::A));
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_sync_b(move || do_sync(&engine, &state, &ui_weak, DeckId::B));
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_loop_a(move || toggle_loop(&engine, &state, &ui_weak, DeckId::A, 8.0));
        }
        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_loop_b(move || toggle_loop(&engine, &state, &ui_weak, DeckId::B, 8.0));
        }

        {
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let db = Arc::clone(&db);
            ui.on_browse_a(move || {
                if let Some(path) = native_open_audio() {
                    load_path_on_deck(&ui_weak, &engine, &state, &db, DeckId::A, path);
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
                    load_path_on_deck(&ui_weak, &engine, &state, &db, DeckId::B, path);
                }
            });
        }

        {
            let engine = Arc::clone(&engine);
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_mixer_changed(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let st = state.lock();
                    Self::send_mixer(&engine, &ui, &st);
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
                let preview = st.preview_in_master;
                let a = st.cue_a;
                let b = st.cue_b;
                drop(st);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_cue_a(a);
                    engine.send(EngineCommand::SetCue {
                        cue_a: a,
                        cue_b: b,
                        blend: ui.get_cue_blend(),
                        hp_vol: ui.get_headphone_vol(),
                        preview_in_master: preview,
                    });
                    if !ui.get_cue_available() && !preview {
                        ui.set_status_banner(
                            "No cue output — enable PREVIEW to hear cue in master.".into(),
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
                let preview = st.preview_in_master;
                let a = st.cue_a;
                let b = st.cue_b;
                drop(st);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_cue_b(b);
                    engine.send(EngineCommand::SetCue {
                        cue_a: a,
                        cue_b: b,
                        blend: ui.get_cue_blend(),
                        hp_vol: ui.get_headphone_vol(),
                        preview_in_master: preview,
                    });
                }
            });
        }

        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            ui.on_preview_toggled(move || {
                let mut st = state.lock();
                st.preview_in_master = !st.preview_in_master;
                let v = st.preview_in_master;
                let a = st.cue_a;
                let b = st.cue_b;
                drop(st);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_preview_in_master(v);
                    engine.send(EngineCommand::SetCue {
                        cue_a: a,
                        cue_b: b,
                        blend: ui.get_cue_blend(),
                        hp_vol: ui.get_headphone_vol(),
                        preview_in_master: v,
                    });
                    ui.set_status_banner(
                        if v {
                            "Preview ON — cued decks audible in master.".into()
                        } else {
                            "Preview OFF.".into()
                        },
                    );
                }
            });
        }

        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            ui.on_kill_toggled(move |key| {
                let mut st = state.lock();
                match key.as_str() {
                    "high-a" => st.kill_high_a = !st.kill_high_a,
                    "mid-a" => st.kill_mid_a = !st.kill_mid_a,
                    "low-a" => st.kill_low_a = !st.kill_low_a,
                    "high-b" => st.kill_high_b = !st.kill_high_b,
                    "mid-b" => st.kill_mid_b = !st.kill_mid_b,
                    "low-b" => st.kill_low_b = !st.kill_low_b,
                    _ => {}
                }
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_kill_high_a(st.kill_high_a);
                    ui.set_kill_mid_a(st.kill_mid_a);
                    ui.set_kill_low_a(st.kill_low_a);
                    ui.set_kill_high_b(st.kill_high_b);
                    ui.set_kill_mid_b(st.kill_mid_b);
                    ui.set_kill_low_b(st.kill_low_b);
                    Self::send_mixer(&engine, &ui, &st);
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
            let ui_weak = ui.as_weak();
            ui.on_library_row_selected(move |idx| {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_library_selected(idx);
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
                    ui.set_library_selected(idx);
                    let st = state.lock();
                    let target = if !st.deck_a.playing {
                        DeckId::A
                    } else if !st.deck_b.playing {
                        DeckId::B
                    } else {
                        DeckId::A
                    };
                    drop(st);
                    load_path_on_deck(&ui_weak, &engine, &state, &db, target, path);
                }
            });
        }

        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_library_row_context(move |idx, x, y| {
                if let Some(ui) = ui_weak.upgrade() {
                    let mut st = state.lock();
                    st.ctx_index = idx;
                    Self::open_ctx(
                        &ui,
                        &mut st,
                        CtxKind::Library,
                        x,
                        y,
                        vec![
                            "Load Deck A",
                            "Load Deck B",
                            "Analyze",
                            "Show in Explorer",
                            "Remove from library",
                        ],
                    );
                }
            });
        }

        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_library_drag_begin(move |idx| {
                if let Some(ui) = ui_weak.upgrade() {
                    let tracks = ui.get_library_tracks();
                    if idx < 0 || idx as usize >= tracks.row_count() {
                        return;
                    }
                    let row = tracks.row_data(idx as usize).unwrap();
                    let mut st = state.lock();
                    st.drag_track_id = Some(row.id.to_string());
                    st.drag_label = row.title.to_string();
                    ui.set_lib_dragging(true);
                    ui.set_lib_drag_label(row.title);
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_library_drag_move(move |x, y| {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_lib_drag_x(x);
                    ui.set_lib_drag_y(y);
                    let st = state.lock();
                    let zone = st.layout.hit_test(x as f64, y as f64);
                    drop(st);
                    update_drop_hover(&ui_weak, zone);
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            let db = Arc::clone(&db);
            ui.on_library_drag_end(move |x, y| {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_lib_dragging(false);
                    update_drop_hover(&ui_weak, DropZone::None);
                    let (id, zone) = {
                        let mut st = state.lock();
                        let id = st.drag_track_id.take();
                        let zone = st.layout.hit_test(x as f64, y as f64);
                        (id, zone)
                    };
                    if let Some(id) = id {
                        if let (DropZone::DeckA | DropZone::DeckB, Some(path)) = (
                            zone,
                            db.lock().get_by_id(&id).ok().flatten().map(|t| PathBuf::from(t.path)),
                        ) {
                            let deck = if zone == DropZone::DeckA {
                                DeckId::A
                            } else {
                                DeckId::B
                            };
                            load_path_on_deck(&ui_weak, &engine, &state, &db, deck, path);
                        }
                    }
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
                    Self::publish_layout(&state, &ui);
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_deck_a_geometry(move |x, y, w, h| {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_deck_a_x(x);
                    ui.set_deck_a_y(y);
                    ui.set_deck_a_w(w);
                    ui.set_deck_a_h(h);
                    Self::publish_layout(&state, &ui);
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_deck_b_geometry(move |x, y, w, h| {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_deck_b_x(x);
                    ui.set_deck_b_y(y);
                    ui.set_deck_b_w(w);
                    ui.set_deck_b_h(h);
                    Self::publish_layout(&state, &ui);
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_library_geometry(move |x, y, w, h| {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_library_x(x);
                    ui.set_library_y(y);
                    ui.set_library_w(w);
                    ui.set_library_h(h);
                    Self::publish_layout(&state, &ui);
                }
            });
        }

        // Seek / jump / nudge / hot cues
        wire_seek(ui, &engine, &state, DeckId::A);
        wire_seek(ui, &engine, &state, DeckId::B);
        {
            let engine = Arc::clone(&engine);
            ui.on_beat_jump_a(move |b| {
                engine.send(EngineCommand::BeatJump {
                    id: DeckId::A,
                    beats: b as f64,
                })
            });
        }
        {
            let engine = Arc::clone(&engine);
            ui.on_beat_jump_b(move |b| {
                engine.send(EngineCommand::BeatJump {
                    id: DeckId::B,
                    beats: b as f64,
                })
            });
        }
        {
            let engine = Arc::clone(&engine);
            ui.on_nudge_a(move |on| {
                engine.send(EngineCommand::SetTempoBend {
                    id: DeckId::A,
                    bend: if on { 0.02 } else { 0.0 },
                })
            });
        }
        {
            let engine = Arc::clone(&engine);
            ui.on_nudge_b(move |on| {
                engine.send(EngineCommand::SetTempoBend {
                    id: DeckId::B,
                    bend: if on { 0.02 } else { 0.0 },
                })
            });
        }
        wire_hot_cue(ui, &engine, &state, &db, DeckId::A);
        wire_hot_cue(ui, &engine, &state, &db, DeckId::B);

        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_deck_a_context(move |x, y| {
                if let Some(ui) = ui_weak.upgrade() {
                    let mut st = state.lock();
                    Self::open_ctx(
                        &ui,
                        &mut st,
                        CtxKind::DeckA,
                        x,
                        y,
                        vec![
                            "Unload",
                            "Half tempo",
                            "Double tempo",
                            "Use BPM as master",
                            "Shift grid earlier",
                            "Shift grid later",
                        ],
                    );
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_deck_b_context(move |x, y| {
                if let Some(ui) = ui_weak.upgrade() {
                    let mut st = state.lock();
                    Self::open_ctx(
                        &ui,
                        &mut st,
                        CtxKind::DeckB,
                        x,
                        y,
                        vec![
                            "Unload",
                            "Half tempo",
                            "Double tempo",
                            "Use BPM as master",
                            "Shift grid earlier",
                            "Shift grid later",
                        ],
                    );
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_wave_a_context(move |p, x, y| {
                if let Some(ui) = ui_weak.upgrade() {
                    let mut st = state.lock();
                    st.ctx_wave_pos = p;
                    Self::open_ctx(
                        &ui,
                        &mut st,
                        CtxKind::WaveA,
                        x,
                        y,
                        vec![
                            "Set cue here",
                            "Jump here",
                            "Loop 4 beats",
                            "Loop 8 beats",
                            "Clear loop",
                        ],
                    );
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            ui.on_wave_b_context(move |p, x, y| {
                if let Some(ui) = ui_weak.upgrade() {
                    let mut st = state.lock();
                    st.ctx_wave_pos = p;
                    Self::open_ctx(
                        &ui,
                        &mut st,
                        CtxKind::WaveB,
                        x,
                        y,
                        vec![
                            "Set cue here",
                            "Jump here",
                            "Loop 4 beats",
                            "Loop 8 beats",
                            "Clear loop",
                        ],
                    );
                }
            });
        }

        {
            let ui_weak = ui.as_weak();
            ui.on_ctx_dismiss(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_ctx_open(false);
                }
            });
        }
        {
            let state = Arc::clone(&state);
            let ui_weak = ui.as_weak();
            let engine = Arc::clone(&engine);
            let db = Arc::clone(&db);
            ui.on_ctx_action(move |idx| {
                handle_ctx_action(&ui_weak, &engine, &state, &db, idx);
            });
        }
    }

    fn install_file_drop(&self) {
        use slint::winit_030::{EventResult, WinitWindowAccessor};

        let ui_weak = self.ui.as_weak();
        let state = Arc::clone(&self.state);
        let engine = Arc::clone(&self.engine);
        let db = Arc::clone(&self.db);

        // HWND capture + OLE registration (timer kept alive).
        #[cfg(windows)]
        {
            let state = Arc::clone(&self.state);
            let ui_weak = self.ui.as_weak();
            let engine = Arc::clone(&self.engine);
            let db = Arc::clone(&self.db);
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(400),
                move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.window().with_winit_window(|winit_window| {
                            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                            if let Ok(handle) = winit_window.window_handle() {
                                if let RawWindowHandle::Win32(h) = handle.as_raw() {
                                    let hwnd = h.hwnd.get() as isize;
                                    let mut st = state.lock();
                                    if st.last_hwnd != hwnd {
                                        st.last_hwnd = hwnd;
                                    }
                                    if !st.ole_active && hwnd != 0 {
                                        let layout = st.shared_layout.clone();
                                        let ui_weak2 = ui_weak.clone();
                                        let engine2 = Arc::clone(&engine);
                                        let state2 = Arc::clone(&state);
                                        let db2 = Arc::clone(&db);
                                        let cb: crate::win_drop::OleDropCallback =
                                            Arc::new(move |ev| {
                                                let ui_weak = ui_weak2.clone();
                                                let engine = Arc::clone(&engine2);
                                                let state = Arc::clone(&state2);
                                                let db = Arc::clone(&db2);
                                                let _ = slint::invoke_from_event_loop(move || {
                                                    handle_ole_event(
                                                        &ui_weak, &engine, &state, &db, ev,
                                                    );
                                                });
                                            });
                                        if crate::win_drop::try_register(hwnd, layout, cb) {
                                            st.ole_active = true;
                                            eprintln!("OLE IDropTarget registered");
                                        }
                                    }
                                }
                            }
                        });
                        ArcLiveApp::publish_layout(&state, &ui);
                    }

                    // Flush coalesced winit multi-file drops.
                    let ready = {
                        let st = state.lock();
                        !st.ole_active && st.drop_acc.ready(Duration::from_millis(40))
                    };
                    if ready {
                        let (zone, paths, playing, protection) = {
                            let mut st = state.lock();
                            let Some((zone, paths, _)) = st.drop_acc.take() else {
                                return;
                            };
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
                    }
                },
            );
            std::mem::forget(timer);
        }

        self.ui.window().on_winit_window_event(move |_win, event| {
            use slint::winit_030::winit::event::WindowEvent;
            let ole = state.lock().ole_active;
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let mut st = state.lock();
                    st.cursor = (position.x, position.y);
                    let dragging = st.drag_track_id.is_some();
                    let scale = st.scale;
                    drop(st);
                    if dragging {
                        let (lx, ly) = physical_to_logical(position.x, position.y, scale);
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_lib_drag_x(lx as f32);
                            ui.set_lib_drag_y(ly as f32);
                        }
                        let zone = {
                            let st = state.lock();
                            st.layout.hit_test(lx, ly)
                        };
                        update_drop_hover(&ui_weak, zone);
                    }
                    EventResult::Propagate
                }
                WindowEvent::MouseInput {
                    state: btn_state,
                    button,
                    ..
                } => {
                    use slint::winit_030::winit::event::{ElementState, MouseButton};
                    if button == &MouseButton::Left && btn_state == &ElementState::Released {
                        let (id, cursor, scale) = {
                            let st = state.lock();
                            (st.drag_track_id.clone(), st.cursor, st.scale)
                        };
                        if id.is_some() {
                            let (lx, ly) = physical_to_logical(cursor.0, cursor.1, scale);
                            // Reuse the Slint drag-end path.
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.invoke_library_drag_end(lx as f32, ly as f32);
                            }
                        }
                    }
                    EventResult::Propagate
                }
                WindowEvent::HoveredFile(path) if !ole => {
                    let mut st = state.lock();
                    if !st.hover_paths.contains(path) {
                        st.hover_paths.push(path.clone());
                    }
                    if let Some((x, y)) = cursor_client_pos(st.last_hwnd) {
                        st.cursor = (x, y);
                    }
                    let zone = st
                        .layout
                        .hit_test_physical(st.cursor.0, st.cursor.1, st.scale);
                    st.hover_zone = zone;
                    drop(st);
                    update_drop_hover(&ui_weak, zone);
                    EventResult::PreventDefault
                }
                WindowEvent::HoveredFileCancelled if !ole => {
                    {
                        let mut st = state.lock();
                        st.hover_paths.clear();
                        st.hover_zone = DropZone::None;
                    }
                    update_drop_hover(&ui_weak, DropZone::None);
                    EventResult::PreventDefault
                }
                WindowEvent::DroppedFile(path) if !ole => {
                    let mut st = state.lock();
                    if let Some((x, y)) = cursor_client_pos(st.last_hwnd) {
                        st.cursor = (x, y);
                    }
                    let (lx, ly) = physical_to_logical(st.cursor.0, st.cursor.1, st.scale);
                    let zone = st.layout.hit_test(lx, ly);
                    st.drop_acc.push(path.clone(), zone, (lx, ly));
                    // Also merge any hover_paths accumulated
                    let pending = std::mem::take(&mut st.hover_paths);
                    for p in pending {
                        st.drop_acc.push(p, zone, (lx, ly));
                    }
                    st.hover_zone = DropZone::None;
                    EventResult::PreventDefault
                }
                _ => EventResult::Propagate,
            }
        });
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
        std::mem::forget(timer);
    }
}

fn toggle_play(
    engine: &AudioEngine,
    state: &Arc<Mutex<AppState>>,
    ui_weak: &Weak<AppWindow>,
    id: DeckId,
) {
    let mut st = state.lock();
    let deck = match id {
        DeckId::A => &mut st.deck_a,
        DeckId::B => &mut st.deck_b,
    };
    if !deck.can_play {
        return;
    }
    deck.playing = !deck.playing;
    let playing = deck.playing;
    drop(st);
    if playing {
        engine.send(EngineCommand::Play(id));
    } else {
        engine.send(EngineCommand::Pause(id));
    }
    if let Some(ui) = ui_weak.upgrade() {
        match id {
            DeckId::A => {
                ui.set_deck_a_playing(playing);
                ui.set_deck_a_status(if playing { "Playing".into() } else { "Paused".into() });
            }
            DeckId::B => {
                ui.set_deck_b_playing(playing);
                ui.set_deck_b_status(if playing { "Playing".into() } else { "Paused".into() });
            }
        }
    }
}

fn do_sync(
    engine: &AudioEngine,
    state: &Arc<Mutex<AppState>>,
    ui_weak: &Weak<AppWindow>,
    follower: DeckId,
) {
    let mut st = state.lock();
    let deck = match follower {
        DeckId::A => &mut st.deck_a,
        DeckId::B => &mut st.deck_b,
    };
    if !deck.can_play {
        return;
    }
    if deck.synced {
        deck.synced = false;
        drop(st);
        engine.send(EngineCommand::SetSync {
            id: follower,
            synced: false,
        });
        if let Some(ui) = ui_weak.upgrade() {
            match follower {
                DeckId::A => ui.set_deck_a_synced(false),
                DeckId::B => ui.set_deck_b_synced(false),
            }
            ui.set_status_banner("Sync off.".into());
        }
        return;
    }
    deck.synced = true;
    drop(st);
    engine.send(EngineCommand::SyncLock { follower });
    if let Some(ui) = ui_weak.upgrade() {
        match follower {
            DeckId::A => ui.set_deck_a_synced(true),
            DeckId::B => ui.set_deck_b_synced(true),
        }
        ui.set_status_banner("Sync: phase locked to the other deck.".into());
    }
}

fn toggle_loop(
    engine: &AudioEngine,
    state: &Arc<Mutex<AppState>>,
    ui_weak: &Weak<AppWindow>,
    id: DeckId,
    beats: f64,
) {
    let mut st = state.lock();
    let deck = match id {
        DeckId::A => &mut st.deck_a,
        DeckId::B => &mut st.deck_b,
    };
    if !deck.can_play || deck.bpm <= 0.0 || deck.duration <= 0.0 {
        return;
    }
    deck.loop_on = !deck.loop_on;
    let on = deck.loop_on;
    let sr = 48000.0f32;
    let playhead = engine.deck_position_frames(id);
    let (start, end) = if on {
        loop_region_frames(playhead, deck.bpm, sr, beats)
    } else {
        (0.0, 0.0)
    };
    let dur_frames = deck.duration * sr as f64;
    if on && dur_frames > 0.0 {
        deck.loop_start_norm = (start / dur_frames) as f32;
        deck.loop_end_norm = (end / dur_frames) as f32;
    } else {
        deck.loop_start_norm = -1.0;
        deck.loop_end_norm = -1.0;
    }
    let ls = deck.loop_start_norm;
    let le = deck.loop_end_norm;
    drop(st);
    engine.send(EngineCommand::SetLoop {
        id,
        enabled: on,
        start,
        end,
    });
    if let Some(ui) = ui_weak.upgrade() {
        match id {
            DeckId::A => {
                ui.set_deck_a_loop_on(on);
                ui.set_deck_a_loop_start(ls);
                ui.set_deck_a_loop_end(le);
            }
            DeckId::B => {
                ui.set_deck_b_loop_on(on);
                ui.set_deck_b_loop_start(ls);
                ui.set_deck_b_loop_end(le);
            }
        }
    }
}

fn wire_seek(ui: &AppWindow, engine: &Arc<AudioEngine>, state: &Arc<Mutex<AppState>>, id: DeckId) {
    let engine = Arc::clone(engine);
    let state = Arc::clone(state);
    match id {
        DeckId::A => {
            ui.on_seek_a(move |p| {
                let st = state.lock();
                let frames = p as f64 * st.deck_a.duration * 48000.0;
                drop(st);
                engine.send(EngineCommand::Seek {
                    id: DeckId::A,
                    frames,
                });
            });
        }
        DeckId::B => {
            ui.on_seek_b(move |p| {
                let st = state.lock();
                let frames = p as f64 * st.deck_b.duration * 48000.0;
                drop(st);
                engine.send(EngineCommand::Seek {
                    id: DeckId::B,
                    frames,
                });
            });
        }
    }
}

fn wire_hot_cue(
    ui: &AppWindow,
    engine: &Arc<AudioEngine>,
    state: &Arc<Mutex<AppState>>,
    db: &Arc<Mutex<LibraryDb>>,
    id: DeckId,
) {
    let engine = Arc::clone(engine);
    let state = Arc::clone(state);
    let db = Arc::clone(db);
    let ui_weak = ui.as_weak();
    let handler = move |slot: i32, shift: bool| {
        let mut st = state.lock();
        let deck = match id {
            DeckId::A => &mut st.deck_a,
            DeckId::B => &mut st.deck_b,
        };
        if !deck.can_play {
            return;
        }
        let sr = engine.sample_rate() as f64;
        if shift {
            let pos = engine.deck_position_frames(id) / sr;
            let s = slot as usize;
            if s < 8 {
                deck.hot_cues[s] = true;
                deck.hot_cue_secs[s] = pos;
                if let Some(tid) = deck.track_id.clone() {
                    let _ = db.lock().set_hot_cue(&tid, slot as i64, pos, &format!("Cue {}", slot + 1));
                }
                let flags: Vec<bool> = deck.hot_cues.to_vec();
                drop(st);
                if let Some(ui) = ui_weak.upgrade() {
                    let model = ModelRc::new(VecModel::from(flags));
                    match id {
                        DeckId::A => ui.set_deck_a_hot_cues(model),
                        DeckId::B => ui.set_deck_b_hot_cues(model),
                    }
                }
            }
        } else {
            let s = slot as usize;
            if s < 8 && deck.hot_cues[s] {
                let frames = deck.hot_cue_secs[s] * sr;
                drop(st);
                engine.send(EngineCommand::Seek { id, frames });
            }
        }
    };
    match id {
        DeckId::A => ui.on_hot_cue_a(handler),
        DeckId::B => ui.on_hot_cue_b(handler),
    }
}

fn handle_ole_event(
    ui_weak: &Weak<AppWindow>,
    engine: &Arc<AudioEngine>,
    state: &Arc<Mutex<AppState>>,
    db: &Arc<Mutex<LibraryDb>>,
    ev: OleDropEvent,
) {
    match ev {
        OleDropEvent::Hover { zone, .. } => update_drop_hover(ui_weak, zone),
        OleDropEvent::Leave => update_drop_hover(ui_weak, DropZone::None),
        OleDropEvent::Drop { zone, paths, client_x, client_y } => {
            let (zone, playing, protection) = {
                let mut st = state.lock();
                let scale = st.scale;
                let (lx, ly) = physical_to_logical(client_x, client_y, scale);
                // Prefer live layout hit over OLE-reported zone (OLE already hit-tested physical).
                let z = if zone != DropZone::None {
                    zone
                } else {
                    st.layout.hit_test(lx, ly)
                };
                let playing = match z {
                    DropZone::DeckA => st.deck_a.playing,
                    DropZone::DeckB => st.deck_b.playing,
                    _ => false,
                };
                (z, playing, st.config.load_protection)
            };
            update_drop_hover(ui_weak, DropZone::None);
            handle_drop_decision(
                ui_weak,
                engine,
                state,
                db,
                decide_drop(zone, &paths, playing, protection),
            );
        }
    }
}

fn handle_ctx_action(
    ui_weak: &Weak<AppWindow>,
    engine: &Arc<AudioEngine>,
    state: &Arc<Mutex<AppState>>,
    db: &Arc<Mutex<LibraryDb>>,
    idx: i32,
) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };
    ui.set_ctx_open(false);
    let (kind, row_idx, wave_pos) = {
        let st = state.lock();
        (st.ctx_kind, st.ctx_index, st.ctx_wave_pos)
    };

    match kind {
        CtxKind::Library => {
            let tracks = ui.get_library_tracks();
            if row_idx < 0 || row_idx as usize >= tracks.row_count() {
                return;
            }
            let row = tracks.row_data(row_idx as usize).unwrap();
            let id = row.id.to_string();
            let path = db.lock().get_by_id(&id).ok().flatten().map(|t| PathBuf::from(t.path));
            match idx {
                0 => {
                    if let Some(p) = path {
                        load_path_on_deck(ui_weak, engine, state, db, DeckId::A, p);
                    }
                }
                1 => {
                    if let Some(p) = path {
                        load_path_on_deck(ui_weak, engine, state, db, DeckId::B, p);
                    }
                }
                2 => {
                    if let Some(p) = path {
                        // Re-analyze by loading to a free deck temporarily via import
                        import_paths(ui_weak, db, &[p]);
                    }
                }
                3 => {
                    if let Some(p) = path {
                        #[cfg(windows)]
                        {
                            let _ = std::process::Command::new("explorer")
                                .args(["/select,", &p.to_string_lossy()])
                                .spawn();
                        }
                    }
                }
                4 => {
                    let _ = db.lock().remove_track(&id);
                    refresh_library(&ui, db, ui.get_library_search().as_str());
                }
                _ => {}
            }
        }
        CtxKind::WaveA | CtxKind::WaveB => {
            let id = if kind == CtxKind::WaveA {
                DeckId::A
            } else {
                DeckId::B
            };
            let st = state.lock();
            let deck = if kind == CtxKind::WaveA {
                &st.deck_a
            } else {
                &st.deck_b
            };
            let dur = deck.duration;
            let bpm = deck.bpm;
            let sr = engine.sample_rate();
            drop(st);
            match idx {
                0 => {
                    let frames = wave_pos as f64 * dur * sr as f64;
                    engine.send(EngineCommand::SetCuePoint { id, frames });
                    ui.set_status_banner("Cue point set.".into());
                }
                1 => {
                    let frames = wave_pos as f64 * dur * sr as f64;
                    engine.send(EngineCommand::Seek { id, frames });
                }
                2 => set_loop_beats(engine, state, ui_weak, id, 4.0),
                3 => set_loop_beats(engine, state, ui_weak, id, 8.0),
                4 => {
                    engine.send(EngineCommand::SetLoop {
                        id,
                        enabled: false,
                        start: 0.0,
                        end: 0.0,
                    });
                    let mut st = state.lock();
                    let deck = match id {
                        DeckId::A => &mut st.deck_a,
                        DeckId::B => &mut st.deck_b,
                    };
                    deck.loop_on = false;
                    deck.loop_start_norm = -1.0;
                    deck.loop_end_norm = -1.0;
                    drop(st);
                    match id {
                        DeckId::A => {
                            ui.set_deck_a_loop_on(false);
                            ui.set_deck_a_loop_start(-1.0);
                            ui.set_deck_a_loop_end(-1.0);
                        }
                        DeckId::B => {
                            ui.set_deck_b_loop_on(false);
                            ui.set_deck_b_loop_start(-1.0);
                            ui.set_deck_b_loop_end(-1.0);
                        }
                    }
                    let _ = bpm;
                }
                _ => {}
            }
        }
        CtxKind::DeckA | CtxKind::DeckB => {
            let id = if kind == CtxKind::DeckA {
                DeckId::A
            } else {
                DeckId::B
            };
            match idx {
                0 => unload_deck(engine, state, ui_weak, id),
                1 => half_double_tempo(engine, state, ui_weak, id, 0.5),
                2 => half_double_tempo(engine, state, ui_weak, id, 2.0),
                3 => {
                    let bpm = {
                        let st = state.lock();
                        match id {
                            DeckId::A => st.deck_a.bpm,
                            DeckId::B => st.deck_b.bpm,
                        }
                    };
                    if bpm > 0.0 {
                        let mut st = state.lock();
                        st.config.master_bpm = bpm;
                        drop(st);
                        engine.send(EngineCommand::SetMasterBpm(bpm));
                        ui.set_master_bpm(bpm as f32);
                        ui.set_master_bpm_text(format!("{bpm:.2}").into());
                    }
                }
                4 => shift_grid(engine, state, ui_weak, id, -0.01),
                5 => shift_grid(engine, state, ui_weak, id, 0.01),
                _ => {}
            }
        }
        CtxKind::None => {}
    }
}

fn set_loop_beats(
    engine: &AudioEngine,
    state: &Arc<Mutex<AppState>>,
    ui_weak: &Weak<AppWindow>,
    id: DeckId,
    beats: f64,
) {
    let mut st = state.lock();
    let deck = match id {
        DeckId::A => &mut st.deck_a,
        DeckId::B => &mut st.deck_b,
    };
    deck.loop_on = false; // force on
    drop(st);
    // turn on
    let mut st = state.lock();
    let deck = match id {
        DeckId::A => &mut st.deck_a,
        DeckId::B => &mut st.deck_b,
    };
    deck.loop_on = true;
    let sr = engine.sample_rate();
    let playhead = engine.deck_position_frames(id);
    let (start, end) = loop_region_frames(playhead, deck.bpm, sr, beats);
    let dur_frames = deck.duration * sr as f64;
    if dur_frames > 0.0 {
        deck.loop_start_norm = (start / dur_frames) as f32;
        deck.loop_end_norm = (end / dur_frames) as f32;
    }
    let ls = deck.loop_start_norm;
    let le = deck.loop_end_norm;
    drop(st);
    engine.send(EngineCommand::SetLoop {
        id,
        enabled: true,
        start,
        end,
    });
    if let Some(ui) = ui_weak.upgrade() {
        match id {
            DeckId::A => {
                ui.set_deck_a_loop_on(true);
                ui.set_deck_a_loop_start(ls);
                ui.set_deck_a_loop_end(le);
            }
            DeckId::B => {
                ui.set_deck_b_loop_on(true);
                ui.set_deck_b_loop_start(ls);
                ui.set_deck_b_loop_end(le);
            }
        }
    }
}

fn unload_deck(
    engine: &AudioEngine,
    state: &Arc<Mutex<AppState>>,
    ui_weak: &Weak<AppWindow>,
    id: DeckId,
) {
    engine.send(EngineCommand::Unload(id));
    let mut st = state.lock();
    match id {
        DeckId::A => st.deck_a = DeckUiState::default(),
        DeckId::B => st.deck_b = DeckUiState::default(),
    }
    drop(st);
    if let Some(ui) = ui_weak.upgrade() {
        match id {
            DeckId::A => {
                ui.set_deck_a_title("".into());
                ui.set_deck_a_artist("".into());
                ui.set_deck_a_status("Empty".into());
                ui.set_deck_a_can_play(false);
                ui.set_deck_a_playing(false);
                ui.set_deck_a_synced(false);
                ui.set_deck_a_peaks(ModelRc::new(VecModel::from(Vec::<f32>::new())));
                ui.set_deck_a_overview(ModelRc::new(VecModel::from(Vec::<f32>::new())));
            }
            DeckId::B => {
                ui.set_deck_b_title("".into());
                ui.set_deck_b_artist("".into());
                ui.set_deck_b_status("Empty".into());
                ui.set_deck_b_can_play(false);
                ui.set_deck_b_playing(false);
                ui.set_deck_b_synced(false);
                ui.set_deck_b_peaks(ModelRc::new(VecModel::from(Vec::<f32>::new())));
                ui.set_deck_b_overview(ModelRc::new(VecModel::from(Vec::<f32>::new())));
            }
        }
    }
}

fn half_double_tempo(
    engine: &AudioEngine,
    state: &Arc<Mutex<AppState>>,
    ui_weak: &Weak<AppWindow>,
    id: DeckId,
    factor: f64,
) {
    let mut st = state.lock();
    let deck = match id {
        DeckId::A => &mut st.deck_a,
        DeckId::B => &mut st.deck_b,
    };
    if deck.bpm <= 0.0 {
        return;
    }
    deck.bpm *= factor;
    let bpm = deck.bpm;
    let synced = deck.synced;
    drop(st);
    engine.send(EngineCommand::SetTrackBpm { id, bpm });
    if synced {
        engine.send(EngineCommand::SyncLock { follower: id });
    }
    if let Some(ui) = ui_weak.upgrade() {
        match id {
            DeckId::A => ui.set_deck_a_bpm(format!("{bpm:.2}").into()),
            DeckId::B => ui.set_deck_b_bpm(format!("{bpm:.2}").into()),
        }
    }
}

fn shift_grid(
    engine: &AudioEngine,
    state: &Arc<Mutex<AppState>>,
    ui_weak: &Weak<AppWindow>,
    id: DeckId,
    delta: f64,
) {
    let mut st = state.lock();
    let deck = match id {
        DeckId::A => &mut st.deck_a,
        DeckId::B => &mut st.deck_b,
    };
    deck.grid_offset = (deck.grid_offset + delta).max(0.0);
    let off = deck.grid_offset;
    let dur = deck.duration.max(0.001);
    let bpm = deck.bpm.max(1.0);
    let beat_norm = (60.0 / bpm) / dur;
    let grid_off_norm = (off / dur) as f32;
    drop(st);
    engine.send(EngineCommand::SetGridOffset {
        id,
        offset_secs: off,
    });
    if let Some(ui) = ui_weak.upgrade() {
        match id {
            DeckId::A => {
                ui.set_deck_a_grid_offset(grid_off_norm);
                ui.set_deck_a_beat_spacing(beat_norm as f32);
            }
            DeckId::B => {
                ui.set_deck_b_grid_offset(grid_off_norm);
                ui.set_deck_b_beat_spacing(beat_norm as f32);
            }
        }
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
        let _ = (confidence, warp_json);

        set_status("Preparing warp");
        let peaks = waveform_peaks(&decoded.samples, 256);
        let overview = waveform_peaks(&decoded.samples, 128);
        let master_bpm = state.lock().config.master_bpm;

        let mut row = track_row_from_meta(&decoded.meta, "Ready");
        row.bpm = bpm;
        let track_id = row.id.clone();
        let mut hot_flags = [false; 8];
        let mut hot_secs = [0.0; 8];
        {
            let db = db.lock();
            let _ = db.upsert_track(&row);
            if let Ok(pid) = db.create_playlist("Default") {
                let _ = db.add_to_playlist(&pid, &row.id, 0);
            }
            let _ = db.set_hot_cue(&row.id, 0, 0.0, "Cue 1");
            if let Ok(cues) = db.get_hot_cues(&row.id) {
                for (slot, pos, _) in cues {
                    if (0..8).contains(&slot) {
                        hot_flags[slot as usize] = true;
                        hot_secs[slot as usize] = pos;
                    }
                }
            }
        }

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
            grid_offset_secs: offset,
        });

        let meta = decoded.meta;
        let duration = meta.duration_secs;
        let beat_spacing = if bpm > 0.0 && duration > 0.0 {
            ((60.0 / bpm) / duration) as f32
        } else {
            0.04
        };
        let grid_off_norm = if duration > 0.0 {
            (offset / duration) as f32
        } else {
            0.0
        };
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
            let hot_model = ModelRc::new(VecModel::from(hot_flags.to_vec()));

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
                deck.track_id = Some(track_id);
                deck.grid_offset = offset;
                deck.hot_cues = hot_flags;
                deck.hot_cue_secs = hot_secs;
                deck.loop_on = false;
                deck.loop_start_norm = -1.0;
                deck.loop_end_norm = -1.0;
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
                    ui.set_deck_a_beat_spacing(beat_spacing);
                    ui.set_deck_a_grid_offset(grid_off_norm);
                    ui.set_deck_a_hot_cues(hot_model);
                    ui.set_deck_a_loop_on(false);
                    ui.set_deck_a_loop_start(-1.0);
                    ui.set_deck_a_loop_end(-1.0);
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
                    ui.set_deck_b_beat_spacing(beat_spacing);
                    ui.set_deck_b_grid_offset(grid_off_norm);
                    ui.set_deck_b_hot_cues(hot_model);
                    ui.set_deck_b_loop_on(false);
                    ui.set_deck_b_loop_start(-1.0);
                    ui.set_deck_b_loop_end(-1.0);
                }
            }
            refresh_library(&ui, &db2, ui.get_library_search().as_str());
            ui.set_status_banner(
                format!("Ready: {} ({bpm:.2} BPM)", meta.title).into(),
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
