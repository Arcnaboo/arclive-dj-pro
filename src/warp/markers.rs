use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpMarker {
    /// Source time in seconds.
    pub source_secs: f64,
    /// Musical beat position.
    pub beat: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WarpMap {
    pub markers: Vec<WarpMarker>,
}

impl WarpMap {
    pub fn from_constant(bpm: f64, duration_secs: f64) -> Self {
        let beats = duration_secs * bpm / 60.0;
        Self {
            markers: vec![
                WarpMarker {
                    source_secs: 0.0,
                    beat: 0.0,
                },
                WarpMarker {
                    source_secs: duration_secs,
                    beat: beats,
                },
            ],
        }
    }

    pub fn add_marker(&mut self, source_secs: f64, beat: f64) {
        self.markers.push(WarpMarker { source_secs, beat });
        self.markers
            .sort_by(|a, b| a.source_secs.partial_cmp(&b.source_secs).unwrap());
    }

    pub fn reset(&mut self, bpm: f64, duration_secs: f64) {
        *self = Self::from_constant(bpm, duration_secs);
    }

    /// Map musical beat → source seconds via piecewise linear markers.
    pub fn beat_to_source(&self, beat: f64) -> f64 {
        if self.markers.len() < 2 {
            return 0.0;
        }
        for w in self.markers.windows(2) {
            let a = &w[0];
            let b = &w[1];
            if beat >= a.beat && beat <= b.beat {
                let t = if (b.beat - a.beat).abs() < 1e-12 {
                    0.0
                } else {
                    (beat - a.beat) / (b.beat - a.beat)
                };
                return a.source_secs + t * (b.source_secs - a.source_secs);
            }
        }
        if beat < self.markers[0].beat {
            return self.markers[0].source_secs;
        }
        self.markers.last().map(|m| m.source_secs).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_map() {
        let map = WarpMap::from_constant(120.0, 60.0);
        assert!((map.beat_to_source(0.0)).abs() < 1e-9);
        assert!((map.beat_to_source(120.0) - 60.0).abs() < 1e-6);
    }
}