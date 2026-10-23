#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossfaderCurve {
    Linear,
    EqualPower,
    SharpCut,
}

impl CrossfaderCurve {
    pub fn next(self) -> Self {
        match self {
            Self::Linear => Self::EqualPower,
            Self::EqualPower => Self::SharpCut,
            Self::SharpCut => Self::Linear,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::EqualPower => "Equal Power",
            Self::SharpCut => "Sharp Cut",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CrossfaderGains {
    pub a: f32,
    pub b: f32,
}

/// `position` 0 = full A, 1 = full B, 0.5 = both.
pub fn crossfader_gains(position: f32, curve: CrossfaderCurve) -> CrossfaderGains {
    let x = position.clamp(0.0, 1.0);
    match curve {
        CrossfaderCurve::Linear => CrossfaderGains {
            a: 1.0 - x,
            b: x,
        },
        CrossfaderCurve::EqualPower => {
            let angle = x * std::f32::consts::FRAC_PI_2;
            CrossfaderGains {
                a: angle.cos(),
                b: angle.sin(),
            }
        }
        CrossfaderCurve::SharpCut => {
            if x < 0.45 {
                CrossfaderGains { a: 1.0, b: 0.0 }
            } else if x > 0.55 {
                CrossfaderGains { a: 0.0, b: 1.0 }
            } else {
                let t = (x - 0.45) / 0.10;
                CrossfaderGains {
                    a: 1.0 - t,
                    b: t,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_power_center() {
        let g = crossfader_gains(0.5, CrossfaderCurve::EqualPower);
        assert!((g.a - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);
        assert!((g.b - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);
    }

    #[test]
    fn linear_ends() {
        let l = crossfader_gains(0.0, CrossfaderCurve::Linear);
        assert!((l.a - 1.0).abs() < 1e-6 && l.b.abs() < 1e-6);
        let r = crossfader_gains(1.0, CrossfaderCurve::Linear);
        assert!((r.b - 1.0).abs() < 1e-6 && r.a.abs() < 1e-6);
    }
}