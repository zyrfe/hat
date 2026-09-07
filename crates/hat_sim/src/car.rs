//! Car types and per-car state. A car is a box on two trucks. See docs/sim/car-model.md.

use hat_units::G;

use crate::params::*;

pub type CarTypeId = u16;
pub type CarId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CarKind {
    Locomotive,
    OpenHopper,
    CoveredHopper,
    Tank,
    Boxcar,
    Gondola,
    Flatcar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Commodity {
    #[default]
    Empty,
    Coal,
    Grain,
    Oil,
    Aggregate,
    Lumber,
    Mixed,
}

#[derive(Clone, Debug)]
pub struct LocoSpec {
    /// Power at the rail, W.
    pub power_rail: f64,
    /// Adhesion coefficient for tractive effort.
    pub adhesion: f64,
    /// Independent brake force at full application, N.
    pub independent_max: f64,
}

#[derive(Clone, Debug)]
pub struct CarType {
    pub name: &'static str,
    pub kind: CarKind,
    /// Center-to-center spacing when coupled with slack centered, m.
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub m_tare: f64,
    pub m_payload_max: f64,
    pub n_axles: u32,
    pub loco: Option<LocoSpec>,
}

pub const TYPE_SWITCHER: CarTypeId = 0;
pub const TYPE_OPEN_HOPPER: CarTypeId = 1;
pub const TYPE_COVERED_HOPPER: CarTypeId = 2;
pub const TYPE_TANK: CarTypeId = 3;
pub const TYPE_BOXCAR: CarTypeId = 4;
pub const TYPE_GONDOLA: CarTypeId = 5;
pub const TYPE_FLATCAR: CarTypeId = 6;
pub const TYPE_ROAD_LOCO: CarTypeId = 7;

impl CarType {
    pub fn is_loco(&self) -> bool {
        self.loco.is_some()
    }

    pub fn standard_library() -> Vec<CarType> {
        let m_switcher = 118_000.0;
        vec![
            CarType {
                name: "Switcher",
                kind: CarKind::Locomotive,
                length: 13.7,
                width: 3.1,
                height: 4.4,
                m_tare: m_switcher,
                m_payload_max: 0.0,
                n_axles: 4,
                loco: Some(LocoSpec { power_rail: 1_000_000.0, adhesion: 0.25, independent_max: 0.25 * m_switcher * G }),
            },
            CarType { name: "Open hopper", kind: CarKind::OpenHopper, length: 16.2, width: 3.2, height: 3.6, m_tare: 24_000.0, m_payload_max: 105_000.0, n_axles: 4, loco: None },
            CarType { name: "Covered hopper", kind: CarKind::CoveredHopper, length: 18.0, width: 3.2, height: 4.6, m_tare: 27_000.0, m_payload_max: 100_000.0, n_axles: 4, loco: None },
            CarType { name: "Tank", kind: CarKind::Tank, length: 18.3, width: 3.2, height: 4.4, m_tare: 30_000.0, m_payload_max: 95_000.0, n_axles: 4, loco: None },
            CarType { name: "Boxcar", kind: CarKind::Boxcar, length: 18.6, width: 3.2, height: 4.7, m_tare: 30_000.0, m_payload_max: 70_000.0, n_axles: 4, loco: None },
            CarType { name: "Gondola", kind: CarKind::Gondola, length: 16.5, width: 3.2, height: 2.6, m_tare: 28_000.0, m_payload_max: 100_000.0, n_axles: 4, loco: None },
            CarType { name: "Flatcar", kind: CarKind::Flatcar, length: 27.4, width: 3.2, height: 1.4, m_tare: 30_000.0, m_payload_max: 70_000.0, n_axles: 4, loco: None },
            CarType {
                name: "Road unit",
                kind: CarKind::Locomotive,
                length: 22.3,
                width: 3.1,
                height: 4.7,
                m_tare: 192_000.0,
                m_payload_max: 0.0,
                n_axles: 6,
                loco: Some(LocoSpec { power_rail: 3_200_000.0, adhesion: 0.30, independent_max: 0.25 * 192_000.0 * G }),
            },
        ]
    }
}

/// Air brake state of one car. Pressures in Pa.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Brake {
    pub p_pipe: f64,
    pub p_aux: f64,
    pub p_cyl: f64,
    pub emergency: bool,
}

impl Brake {
    pub fn charged() -> Self {
        Brake { p_pipe: PIPE_REF, p_aux: PIPE_REF, p_cyl: 0.0, emergency: false }
    }
    pub fn bled() -> Self {
        Brake::default()
    }
    pub fn is_bled(&self) -> bool {
        self.p_aux < BLED_THRESHOLD
    }
}

#[derive(Clone, Debug)]
pub struct CarState {
    pub id: CarId,
    pub type_id: CarTypeId,
    /// Path coordinate of the car center, m. Increases toward the head of the train.
    pub x: f64,
    /// Velocity along +x, m/s.
    pub v: f64,
    pub m_payload: f64,
    pub commodity: Commodity,
    pub brake: Brake,
    /// 0 released, 1 fully applied.
    pub hand_brake: f64,
    /// [toward head, toward tail]
    pub knuckle_open: [bool; 2],
    pub damage: f64,
    pub derailed: bool,
    /// Scenario tag: destination track.
    pub dest: Option<u32>,
    /// True when the car's own front points toward the head of the train.
    pub facing_head: bool,
    /// Sim time before which each end refuses to couple, set by a pin pull. [head end, tail end]
    pub no_couple_until: [f64; 2],
    /// The car each end was just parted from. That pair will not re-couple until they have
    /// actually separated, so a loose car can be shoved without re-locking the knuckle.
    pub no_couple_with: [Option<CarId>; 2],
    /// Industry whose facility loaded the payload, for freight settlement.
    pub origin: Option<u32>,
    /// Payload unloaded since the last Unloaded event, kg.
    pub delivered_acc: f64,
    /// Tractive work done at the rail by this car, J. Locomotives only. Fuel follows from it.
    pub work_j: f64,
    /// Share of full power the prime mover is actually delivering, 0..1. Follows the notch
    /// with a lag: a diesel loads up over a second or two, it does not step.
    pub power: f64,
}

impl CarState {
    pub fn new(id: CarId, type_id: CarTypeId) -> Self {
        CarState {
            id,
            type_id,
            x: 0.0,
            v: 0.0,
            m_payload: 0.0,
            commodity: Commodity::Empty,
            brake: Brake::bled(),
            hand_brake: 0.0,
            knuckle_open: [false, false],
            damage: 0.0,
            derailed: false,
            dest: None,
            facing_head: true,
            no_couple_until: [0.0, 0.0],
            no_couple_with: [None, None],
            origin: None,
            delivered_acc: 0.0,
            work_j: 0.0,
            power: 0.0,
        }
    }

    pub fn mass(&self, t: &CarType) -> f64 {
        t.m_tare + self.m_payload
    }
}
