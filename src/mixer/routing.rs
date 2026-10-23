#[derive(Debug, Clone, Copy)]
pub struct CueRouting {
    pub cue_a: bool,
    pub cue_b: bool,
    /// 0 = cue only, 1 = master only
    pub cue_master_blend: f32,
    pub headphone_volume: f32,
    pub cue_device_available: bool,
    /// When no separate cue device, mix cued decks into master at headphone volume.
    pub preview_in_master: bool,
}

impl Default for CueRouting {
    fn default() -> Self {
        Self {
            cue_a: false,
            cue_b: false,
            cue_master_blend: 0.0,
            headphone_volume: 0.8,
            cue_device_available: false,
            preview_in_master: false,
        }
    }
}

impl CueRouting {
    /// Build headphone stereo from deck cue buses and master.
    /// When no cue device exists, returns silence (never leak into master unless preview).
    pub fn mix_headphones(
        &self,
        cue_l: f32,
        cue_r: f32,
        master_l: f32,
        master_r: f32,
    ) -> (f32, f32) {
        if !self.cue_device_available {
            return (0.0, 0.0);
        }
        let blend = self.cue_master_blend.clamp(0.0, 1.0);
        let vol = self.headphone_volume.clamp(0.0, 1.0);
        let l = (cue_l * (1.0 - blend) + master_l * blend) * vol;
        let r = (cue_r * (1.0 - blend) + master_r * blend) * vol;
        (l, r)
    }

    /// Preview mix: replace/blend master with cued deck audio on a single device.
    pub fn apply_preview(
        &self,
        master_l: f32,
        master_r: f32,
        cue_l: f32,
        cue_r: f32,
        any_cue: bool,
    ) -> (f32, f32) {
        if self.cue_device_available || !self.preview_in_master || !any_cue {
            return (master_l, master_r);
        }
        let blend = self.cue_master_blend.clamp(0.0, 1.0);
        let vol = self.headphone_volume.clamp(0.0, 1.0);
        let l = master_l * blend + cue_l * (1.0 - blend) * vol;
        let r = master_r * blend + cue_r * (1.0 - blend) * vol;
        (l, r)
    }
}
