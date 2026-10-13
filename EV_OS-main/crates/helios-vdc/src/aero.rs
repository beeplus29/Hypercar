//! Active-aero mode strategy.
//!
//! LowDrag is a drag-reduction state: it is only allowed when the car is
//! stable, nearly straight and at high demand; it is cancelled the instant
//! the driver brakes or lateral acceleration builds. Airbrake is entered on
//! heavy braking at high speed. A minimum dwell prevents mode chatter that the
//! wing actuator (full travel ~0.35 s) cannot follow.

use helios_core::consts::{DEG, G, KPH};
use helios_core::{AeroMode, DriveMode};

#[derive(Clone, Copy, Debug)]
pub struct AeroInputs {
    pub mode: DriveMode,
    pub speed: f32,
    /// Normalised longitudinal demand Fx/(m g): >0 drive, <0 braking.
    pub fx_demand_g: f32,
    pub ay: f32,
    pub steer_road: f32,
    pub yaw_error: f32,
    pub brake_pedal: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct AeroStrategy {
    mode: AeroMode,
    dwell_s: f32,
    pub min_dwell_s: f32,
}

impl Default for AeroStrategy {
    fn default() -> Self {
        Self { mode: AeroMode::HighDownforce, dwell_s: 0.0, min_dwell_s: 0.3 }
    }
}

impl AeroStrategy {
    pub fn mode(&self) -> AeroMode {
        self.mode
    }

    pub fn step(&mut self, i: &AeroInputs, dt: f32) -> AeroMode {
        self.dwell_s += dt;
        let stable = libm::fabsf(i.ay) < 3.0 && libm::fabsf(i.yaw_error) < 0.05;
        let straight = libm::fabsf(i.steer_road) < 2.0 * DEG;
        let braking_hard = i.fx_demand_g < -0.6 || i.brake_pedal > 0.5;
        let airbrake = braking_hard && i.speed > 150.0 * KPH;
        let low_drag_ok = |speed_min: f32, demand_min: f32| {
            stable && straight && i.speed > speed_min && i.fx_demand_g > demand_min && i.brake_pedal < 0.02
        };
        let target = match i.mode {
            DriveMode::Track | DriveMode::Sport => {
                if airbrake {
                    AeroMode::Airbrake
                } else if low_drag_ok(120.0 * KPH, 0.5) {
                    AeroMode::LowDrag
                } else {
                    AeroMode::HighDownforce
                }
            }
            DriveMode::Drift => {
                // Drift mode wants high downforce to stabilize the slide, no DRS.
                if airbrake {
                    AeroMode::Airbrake
                } else {
                    AeroMode::HighDownforce
                }
            }
            DriveMode::Range | DriveMode::Road => {
                if airbrake {
                    AeroMode::Airbrake
                } else if stable && i.speed > 80.0 * KPH && libm::fabsf(i.ay) < 0.3 * G && i.brake_pedal < 0.02 {
                    AeroMode::LowDrag
                } else {
                    AeroMode::HighDownforce
                }
            }
            DriveMode::Wet | DriveMode::Limp => {
                if airbrake {
                    AeroMode::Airbrake
                } else {
                    AeroMode::HighDownforce
                }
            }
        };
        // Safety exits bypass the dwell timer: leaving LowDrag when unstable or braking.
        let safety_exit = self.mode == AeroMode::LowDrag && (!stable || i.brake_pedal >= 0.02);
        if target != self.mode && (self.dwell_s >= self.min_dwell_s || safety_exit) {
            self.mode = target;
            self.dwell_s = 0.0;
        }
        self.mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> AeroInputs {
        AeroInputs {
            mode: DriveMode::Track,
            speed: 70.0,
            fx_demand_g: 0.9,
            ay: 0.2,
            steer_road: 0.0,
            yaw_error: 0.0,
            brake_pedal: 0.0,
        }
    }

    #[test]
    fn straight_full_throttle_goes_low_drag_then_airbrake() {
        let mut a = AeroStrategy::default();
        let i = base();
        for _ in 0..400 {
            a.step(&i, 0.001);
        }
        assert_eq!(a.mode(), AeroMode::LowDrag);
        let mut j = base();
        j.brake_pedal = 0.9;
        j.fx_demand_g = -1.2;
        assert_eq!(a.step(&j, 0.001), AeroMode::Airbrake, "brake must exit LowDrag immediately");
    }

    #[test]
    fn lateral_load_cancels_low_drag_immediately() {
        let mut a = AeroStrategy::default();
        for _ in 0..400 {
            a.step(&base(), 0.001);
        }
        let mut j = base();
        j.ay = 8.0;
        j.steer_road = 4.0 * DEG;
        assert_eq!(a.step(&j, 0.001), AeroMode::HighDownforce);
    }

    #[test]
    fn wet_never_low_drag() {
        let mut a = AeroStrategy::default();
        let mut i = base();
        i.mode = DriveMode::Wet;
        for _ in 0..1000 {
            assert_ne!(a.step(&i, 0.001), AeroMode::LowDrag);
        }
    }
}
