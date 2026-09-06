//! Events emitted by the simulation for audio, scoring and UI.

use glam::DVec2;

use crate::{CarId, NodeId, TrainId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerailCause {
    SplitSwitch,
    Collision,
    Overspeed,
    Bumper,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SimEvent {
    /// Slack ran out or a travel stop was hit between two coupled cars.
    CouplerImpact { pos: DVec2, rel_speed: f64, mass: f64 },
    Coupled { pos: DVec2, rel_speed: f64, train: TrainId, hard: bool, a: CarId, b: CarId },
    Bumped { pos: DVec2, rel_speed: f64 },
    Uncoupled { pos: DVec2, train: TrainId, new_train: TrainId },
    KnuckleBreak { pos: DVec2, train: TrainId, new_train: TrainId },
    PipeVent { pos: DVec2, train: TrainId },
    Emergency { pos: DVec2, train: TrainId },
    Derail { pos: DVec2, cause: DerailCause, train: TrainId },
    BumperHit { pos: DVec2, speed: f64, train: TrainId },
    SwitchThrown { node: NodeId, pos: DVec2 },
    HandBrake { car: CarId, on: bool, pos: DVec2 },
    Bled { car: CarId, pos: DVec2 },
    HosesConnected { train: TrainId, pos: DVec2 },
    AirBottled { train: TrainId, pos: DVec2 },
    /// A car finished loading at a facility.
    Loaded { car: CarId, pos: DVec2, mass: f64, commodity: crate::Commodity, industry: Option<u32> },
    /// A car finished unloading at a facility. `mass` is what came out of it here.
    Unloaded { car: CarId, pos: DVec2, mass: f64, commodity: crate::Commodity, industry: Option<u32>, origin: Option<u32> },
    /// A wreck crew put a derailed train back on the rails.
    Rerailed { train: TrainId, pos: DVec2, cars: usize },
}
