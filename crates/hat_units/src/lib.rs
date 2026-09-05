//! SI conventions for HAT.
//!
//! Everything in the simulation is SI: m, kg, s, N, Pa, W, rad. This crate holds the
//! constants used to import real-world figures and the formatters used to display values
//! in the player's chosen unit system. Nothing else in the workspace knows about miles.

/// Standard gravity, m/s².
pub const G: f64 = 9.806_65;

/// Exact conversion constants. Multiply a customary quantity by the constant to get SI.
pub mod consts {
    /// 1 mile in meters.
    pub const MILE: f64 = 1609.344;
    /// 1 foot in meters.
    pub const FOOT: f64 = 0.3048;
    /// 1 inch in meters.
    pub const INCH: f64 = 0.0254;
    /// 1 mph in m/s.
    pub const MPH: f64 = 0.447_04;
    /// 1 ft/s in m/s.
    pub const FOOT_PER_SECOND: f64 = 0.3048;
    /// 1 km/h in m/s.
    pub const KPH: f64 = 1.0 / 3.6;
    /// 1 pound-force in newtons.
    pub const LBF: f64 = 4.448_221_615_260_5;
    /// 1 pound (mass) in kilograms.
    pub const LB: f64 = 0.453_592_37;
    /// 1 short ton (2000 lb) in kilograms.
    pub const SHORT_TON: f64 = 907.184_74;
    /// 1 long ton (2240 lb) in kilograms.
    pub const LONG_TON: f64 = 1016.046_908_8;
    /// 1 metric tonne in kilograms.
    pub const TONNE: f64 = 1000.0;
    /// 1 psi in pascals.
    pub const PSI: f64 = 6894.757_293_168;
    /// 1 kPa in pascals.
    pub const KPA: f64 = 1000.0;
    /// 1 mechanical horsepower in watts.
    pub const HP: f64 = 745.699_871_582_27;
    /// 1 US gallon in cubic meters.
    pub const US_GALLON: f64 = 0.003_785_411_784;
    /// 1 US bushel in cubic meters.
    pub const BUSHEL: f64 = 0.035_239_07;
}

/// Radius in meters of a curve of `degrees` degrees of curvature (100 ft chord definition).
pub fn radius_from_degree_of_curve(degrees: f64) -> f64 {
    // chord / (2 sin(D/2)) with a 100 ft chord.
    let chord = 100.0 * consts::FOOT;
    chord / (2.0 * (degrees.to_radians() / 2.0).sin())
}

/// Degrees of curvature of a curve with radius `r` meters (100 ft chord definition).
pub fn degree_of_curve_from_radius(r: f64) -> f64 {
    let chord = 100.0 * consts::FOOT;
    2.0 * (chord / (2.0 * r)).asin().to_degrees()
}

/// Which unit system the player wants to see.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnitSystem {
    #[default]
    Metric,
    UsCustomary,
}

impl UnitSystem {
    pub fn toggle(self) -> Self {
        match self {
            UnitSystem::Metric => UnitSystem::UsCustomary,
            UnitSystem::UsCustomary => UnitSystem::Metric,
        }
    }
}

/// Display formatting. The only code in the workspace that knows which system is active.
pub mod fmt {
    use super::{consts, UnitSystem};

    pub fn speed(v_mps: f64, u: UnitSystem) -> String {
        match u {
            UnitSystem::Metric => format!("{:.1} km/h", v_mps / consts::KPH),
            UnitSystem::UsCustomary => format!("{:.1} mph", v_mps / consts::MPH),
        }
    }

    pub fn length(m: f64, u: UnitSystem) -> String {
        match u {
            UnitSystem::Metric => {
                if m.abs() >= 1000.0 {
                    format!("{:.2} km", m / 1000.0)
                } else {
                    format!("{:.0} m", m)
                }
            }
            UnitSystem::UsCustomary => {
                let ft = m / consts::FOOT;
                if ft.abs() >= 5280.0 {
                    format!("{:.2} mi", m / consts::MILE)
                } else {
                    format!("{:.0} ft", ft)
                }
            }
        }
    }

    pub fn mass(kg: f64, u: UnitSystem) -> String {
        match u {
            UnitSystem::Metric => format!("{:.0} t", kg / consts::TONNE),
            UnitSystem::UsCustomary => format!("{:.0} tons", kg / consts::SHORT_TON),
        }
    }

    pub fn pressure(pa: f64, u: UnitSystem) -> String {
        match u {
            UnitSystem::Metric => format!("{:.0} kPa", pa / consts::KPA),
            UnitSystem::UsCustomary => format!("{:.0} psi", pa / consts::PSI),
        }
    }

    pub fn force(n: f64, u: UnitSystem) -> String {
        match u {
            UnitSystem::Metric => format!("{:.0} kN", n / 1000.0),
            UnitSystem::UsCustomary => format!("{:.0} klbf", n / consts::LBF / 1000.0),
        }
    }

    pub fn grade(ratio: f64, _u: UnitSystem) -> String {
        format!("{:.2} %", ratio * 100.0)
    }

    pub fn duration(s: f64) -> String {
        let s = s.max(0.0) as u64;
        let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
        if h > 0 {
            format!("{h}:{m:02}:{sec:02}")
        } else {
            format!("{m}:{sec:02}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degree_of_curve_round_trips() {
        let r = radius_from_degree_of_curve(10.0);
        assert!((r - 174.7).abs() < 0.5, "10 degree curve ~175 m, got {r}");
        let d = degree_of_curve_from_radius(r);
        assert!((d - 10.0).abs() < 1e-9);
    }

    #[test]
    fn formats_both_systems() {
        assert_eq!(fmt::speed(consts::MPH * 4.0, UnitSystem::UsCustomary), "4.0 mph");
        assert_eq!(fmt::pressure(90.0 * consts::PSI, UnitSystem::UsCustomary), "90 psi");
        assert_eq!(fmt::pressure(620_000.0, UnitSystem::Metric), "620 kPa");
        assert_eq!(fmt::duration(3725.0), "1:02:05");
    }
}
