use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait};

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
}

pub fn list_output_devices() -> Result<Vec<DeviceInfo>> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let mut out = Vec::new();
    let devices = host
        .output_devices()
        .map_err(|e| anyhow!("list output devices: {e}"))?;
    for d in devices {
        let name = d.name().unwrap_or_else(|_| "Unknown".into());
        let is_default = name == default_name;
        out.push(DeviceInfo { name, is_default });
    }
    if out.is_empty() {
        return Err(anyhow!("no output devices found"));
    }
    Ok(out)
}