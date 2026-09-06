//! Tunable physical parameters. All SI. Reasoning and sources in docs/reference.md.

// Air brake
pub const PIPE_REF: f64 = 620_000.0;
pub const FULL_SERVICE_CYL: f64 = 440_000.0;
/// Pipe pressure at a full service application.
pub const FULL_SERVICE_PIPE: f64 = 440_000.0;
pub const EMERGENCY_CYL: f64 = 530_000.0;
pub const CYL_PER_PIPE_REDUCTION: f64 = 2.5;
/// Propagation speed of a service reduction along the pipe, m/s.
pub const C_SERVICE: f64 = 150.0;
/// Propagation speed of an emergency application, m/s.
pub const C_EMERGENCY: f64 = 280.0;
/// Local pipe reduction rate during a service application, Pa/s.
pub const PIPE_SERVICE_RATE: f64 = 100_000.0;
/// Local pipe venting rate during emergency, Pa/s.
pub const PIPE_EMERGENCY_RATE: f64 = 600_000.0;
/// Pipe charging rate at the locomotive, Pa/s. Falls with distance.
pub const PIPE_CHARGE_RATE: f64 = 30_000.0;
/// Reservoir charging rate from the pipe, Pa/s.
pub const AUX_CHARGE_RATE: f64 = 12_000.0;
/// A pipe falling faster than this triggers an emergency application, Pa/s.
pub const EMERGENCY_TRIGGER_RATE: f64 = 200_000.0;
pub const CYL_APPLY_TAU: f64 = 2.5;
pub const CYL_RELEASE_TAU: f64 = 6.0;
/// Reservoir and cylinder leakage, Pa/s.
pub const LEAK_RATE: f64 = 300.0;
/// Below this reservoir pressure a car has no air brake.
pub const BLED_THRESHOLD: f64 = 50_000.0;

// Couplers
/// Half of the free slack per coupler pair, m. Total free slack is twice this.
pub const SLACK_HALF: f64 = 0.0125;
pub const DRAFT_GEAR_K: f64 = 1.5e7;
pub const DRAFT_GEAR_C: f64 = 6.0e5;
pub const DRAFT_GEAR_TRAVEL: f64 = 0.075;
pub const STOP_K: f64 = 1.5e8;
pub const KNUCKLE_BREAK: f64 = 1.8e6;
pub const COUPLE_MAX_SPEED: f64 = 1.8;
/// Below this closing speed the knuckle does not lock: the cars just touch.
pub const MIN_COUPLE_SPEED: f64 = 0.08;
pub const COLLISION_DERAIL_SPEED: f64 = 5.0;
pub const BUMP_RESTITUTION: f64 = 0.2;
pub const CONTACT_EPS: f64 = 0.02;
/// Seconds after a pin pull during which the parted ends will not re-couple.
pub const UNCOUPLE_GRACE: f64 = 2.0;
/// Deeper than this is not a contact but two ends back to back.
pub const MAX_PENETRATION: f64 = 0.5;
/// Contact spring between touching, uncoupled ends: stiffness N/m and damping N·s/m.
pub const CONTACT_K: f64 = 1.5e7;
pub const CONTACT_C: f64 = 6.0e5;
/// Penetration beyond which the much stiffer stop takes over.
pub const CONTACT_STOP_AT: f64 = 0.05;

// Resistance (modified Davis, SI form). R = m (A + B v) + C n_axles + D v² + m K curvature
pub const DAVIS_A: f64 = 0.0045;
pub const DAVIS_B: f64 = 1.1e-4;
pub const DAVIS_C: f64 = 89.0;
pub const DAVIS_D: f64 = 1.6;
pub const CURVE_RESISTANCE: f64 = 7.0;
pub const STARTING_RESISTANCE_FACTOR: f64 = 2.0;

// Braking
/// Shoe retarding force at full service as a fraction of tare weight.
pub const BRAKE_RATIO: f64 = 0.105;
/// Wheel slide limit as a fraction of gross weight.
pub const BRAKE_ADHESION: f64 = 0.15;
/// Hand brake holding force as a fraction of gross weight.
pub const HAND_BRAKE_RATIO: f64 = 0.02;

// Derailment
pub const BUMPER_DAMAGE_SPEED: f64 = 1.0;
pub const OVERSPEED_DERAIL_FACTOR: f64 = 1.5;
pub const OVERLAP_DERAIL: f64 = 0.3;
/// Damage a derailed car takes: a flat amount plus a share per m/s it was doing.
pub const DERAIL_DAMAGE: f64 = 0.5;
pub const DERAIL_DAMAGE_PER_MPS: f64 = 0.2;

pub const V_EPS: f64 = 1e-3;

/// Deceleration a retarder applies to a car above its release speed, m/s².
pub const RETARDER_DECEL: f64 = 1.5;
/// Within this distance of standing cars a retarder releases at the close speed instead.
pub const RETARDER_CLOSE_GAP: f64 = 120.0;
pub const RETARDER_CLOSE_SPEED: f64 = 1.6;
