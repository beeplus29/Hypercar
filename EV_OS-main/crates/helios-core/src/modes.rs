//! Drive modes and their calibration profiles.

use crate::consts::{DEG, KPH};

/// Driver-selectable modes (plus `Limp`, which only the VSM can impose).
/// Numeric values are on the wire (CAN, DDS, VHAL `HELIOS_DRIVE_MODE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum DriveMode {
    Range = 0,
    #[default]
    Road = 1,
    Sport = 2,
    Track = 3,
    Drift = 4,
    Wet = 5,
    Limp = 6,
}

impl DriveMode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Range),
            1 => Some(Self::Road),
            2 => Some(Self::Sport),
            3 => Some(Self::Track),
            4 => Some(Self::Drift),
            5 => Some(Self::Wet),
            6 => Some(Self::Limp),
            _ => None,
        }
    }
    /// Limp is never driver-selectable.
    pub const fn driver_selectable(self) -> bool {
        !matches!(self, DriveMode::Limp)
    }

    pub const fn profile(self) -> ModeProfile {
        match self {
            DriveMode::Range => ModeProfile {
                power_fraction: 0.35,
                regen_fraction: 1.0,
                understeer_gradient: 0.0030,
                max_sideslip: 3.0 * DEG,
                slip_target: 0.06,
                mu_assumed: 1.2,
                vmax: 200.0 * KPH,
                adaptive_aero: false,
                pedal_gamma: 1.6,
                front_bias: 0.5, // Even split for efficiency
            },
            DriveMode::Road => ModeProfile {
                power_fraction: 0.65,
                regen_fraction: 0.8,
                understeer_gradient: 0.0022,
                max_sideslip: 4.0 * DEG,
                slip_target: 0.08,
                mu_assumed: 1.3,
                vmax: 300.0 * KPH,
                adaptive_aero: false,
                pedal_gamma: 1.4,
                front_bias: 0.45, // Slight rear bias
            },
            DriveMode::Sport => ModeProfile {
                power_fraction: 0.85,
                regen_fraction: 0.9,
                understeer_gradient: 0.0015,
                max_sideslip: 10.0 * DEG,
                slip_target: 0.12,
                mu_assumed: 1.4,
                vmax: 350.0 * KPH,
                adaptive_aero: true,
                pedal_gamma: 1.2,
                front_bias: 0.3, // Strong rear bias
            },
            DriveMode::Track => ModeProfile {
                power_fraction: 1.0,
                regen_fraction: 1.0,
                understeer_gradient: 0.0012,
                max_sideslip: 6.0 * DEG,
                slip_target: 0.10,
                mu_assumed: 1.5,
                vmax: 412.0 * KPH, // Nevera top speed
                adaptive_aero: true,
                pedal_gamma: 1.0,
                front_bias: 0.45,
            },
            DriveMode::Drift => ModeProfile {
                power_fraction: 1.0,
                regen_fraction: 0.2, // Low regen to not upset the car during drift
                understeer_gradient: -0.0020, // Oversteer setup
                max_sideslip: 45.0 * DEG, // Allow extreme slip angles
                slip_target: 0.25, // Allow lots of wheelspin
                mu_assumed: 1.0, // Assumes slipping tyre
                vmax: 300.0 * KPH,
                adaptive_aero: true,
                pedal_gamma: 1.0,
                front_bias: 0.0, // 100% rear torque vectoring
            },
            DriveMode::Wet => ModeProfile {
                power_fraction: 0.40,
                regen_fraction: 0.5,
                understeer_gradient: 0.0030,
                max_sideslip: 2.5 * DEG,
                slip_target: 0.05,
                mu_assumed: 0.7,
                vmax: 250.0 * KPH,
                adaptive_aero: false,
                pedal_gamma: 1.8,
                front_bias: 0.5,
            },
            DriveMode::Limp => ModeProfile {
                power_fraction: 0.15,
                regen_fraction: 0.3,
                understeer_gradient: 0.0035,
                max_sideslip: 2.0 * DEG,
                slip_target: 0.05,
                mu_assumed: 0.8,
                vmax: 80.0 * KPH,
                adaptive_aero: false,
                pedal_gamma: 1.8,
                front_bias: 0.5,
            },
        }
    }
}

/// Calibration that changes per drive mode.
#[derive(Clone, Copy, Debug)]
pub struct ModeProfile {
    /// Fraction of the system peak drive power available.
    pub power_fraction: f32,
    /// Fraction of the regen capability used for brake blending.
    pub regen_fraction: f32,
    /// Target understeer gradient K_us for the yaw-rate reference, rad/(m/s^2).
    pub understeer_gradient: f32,
    /// Sideslip angle beyond which the stability term overrides yaw tracking, rad.
    pub max_sideslip: f32,
    /// Traction-control target longitudinal slip ratio.
    pub slip_target: f32,
    /// Friction coefficient assumed for yaw-rate saturation and load estimates.
    pub mu_assumed: f32,
    /// Software speed governor, m/s.
    pub vmax: f32,
    /// Allow automatic low-drag / airbrake transitions.
    pub adaptive_aero: bool,
    /// Accelerator progression exponent: demand = pedal^gamma.
    pub pedal_gamma: f32,
    /// Target torque split to the front axle (0.0 = 100% RWD, 1.0 = 100% FWD).
    pub front_bias: f32,
}

/// Active aero configuration (rear wing + front flaps + underbody flap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AeroMode {
    /// Wing flat, front flaps closed, diffuser stall flap open: minimum CdA.
    LowDrag = 0,
    /// Maximum downforce at the design balance.
    #[default]
    HighDownforce = 1,
    /// Wing at maximum incidence for braking; balance shifts rearward.
    Airbrake = 2,
}

impl AeroMode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::LowDrag),
            1 => Some(Self::HighDownforce),
            2 => Some(Self::Airbrake),
            _ => None,
        }
    }
}
