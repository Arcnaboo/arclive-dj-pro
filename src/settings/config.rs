use crate::mixer::crossfader::CrossfaderCurve;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub master_bpm: f64,
    pub window_width: f32,
    pub window_height: f32,
    pub load_protection: bool,
    pub crossfader_curve: String,
    pub library_collapsed: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            master_bpm: 146.0,
            window_width: 1440.0,
            window_height: 900.0,
            load_protection: true,
            crossfader_curve: CrossfaderCurve::EqualPower.label().into(),
            library_collapsed: false,
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ArcLive")
        .join("config.json")
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(cfg: &AppConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = serde_json::to_string_pretty(cfg)?;
    std::fs::write(path, s)?;
    Ok(())
}