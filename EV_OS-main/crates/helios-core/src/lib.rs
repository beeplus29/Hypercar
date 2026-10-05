//! HELIOS shared vehicle types.
//!
//! `#![no_std]`, no heap, no `unsafe`. Everything in here is consumed by the
//! VCU firmware (Cortex-M7, f32 FPU) as well as the host digital twin, so the
//! numeric type for control-side quantities is `f32`.
#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
extern crate std;

pub mod faults;
pub mod gear;
pub mod math;
pub mod modes;
pub mod params;

pub use faults::{Fault, FaultSet};
pub use gear::Gear;
pub use modes::{AeroMode, DriveMode, ModeProfile};
pub use params::{PowertrainLayout, VehicleParams};

/// Physical constants (SI).
pub mod consts {
    /// Standard gravity, m/s^2 (CGPM 1901 conventional value).
    pub const G: f32 = 9.806_65;
    /// ISA sea-level air density, kg/m^3.
    pub const RHO_AIR_ISA: f32 = 1.225;
    /// Degrees to radians.
    pub const DEG: f32 = core::f32::consts::PI / 180.0;
    /// km/h to m/s.
    pub const KPH: f32 = 1.0 / 3.6;
    /// 0 degC in kelvin.
    pub const T0_K: f32 = 273.15;
}

/// Wheel position. Index order FL, FR, RL, RR is fixed across the whole stack
/// (CAN matrix, DDS IDL arrays, VHAL wheel areas mapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Corner {
    FrontLeft = 0,
    FrontRight = 1,
    RearLeft = 2,
    RearRight = 3,
}

impl Corner {
    pub const ALL: [Corner; 4] = [Corner::FrontLeft, Corner::FrontRight, Corner::RearLeft, Corner::RearRight];

    #[inline]
    pub const fn idx(self) -> usize {
        self as usize
    }
    #[inline]
    pub const fn is_front(self) -> bool {
        matches!(self, Corner::FrontLeft | Corner::FrontRight)
    }
    #[inline]
    pub const fn is_left(self) -> bool {
        matches!(self, Corner::FrontLeft | Corner::RearLeft)
    }
    /// Sign of the lateral position in the ISO 8855 body frame (y positive left).
    #[inline]
    pub const fn y_sign(self) -> f32 {
        if self.is_left() { 1.0 } else { -1.0 }
    }
    /// AOSP `VehicleAreaWheel` bit for this corner.
    #[inline]
    pub const fn aosp_wheel_area(self) -> i32 {
        match self {
            Corner::FrontLeft => 0x1,
            Corner::FrontRight => 0x2,
            Corner::RearLeft => 0x4,
            Corner::RearRight => 0x8,
        }
    }
}

/// Fixed-size per-wheel container.
pub type PerCorner<T> = [T; 4];
