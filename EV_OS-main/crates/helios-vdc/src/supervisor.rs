//! Safety Supervisor (Firewall) for Central Compute Advisory Commands.
//!
//! Architecture:
//! - Advisory Tier (QM / Non-ASIL): NVIDIA Jetson AGX Thor / Orin.
//!   Calculates autonomous paths, ADAS vision targets, or AI torque vectoring bias.
//! - Supervisory Firewall (ISO 26262 ASIL D): NXP GoldBox-3 (S32G3) / VCU safety core.
//!   Sanity-checks, plausibility-filters, and clamps advisory demands before they
//!   can reach physical actuators.
//! - Actuation Tier (Hard Real-Time Execution): AEM VCU 300 / Inverters (Helix CTI-4).
//!
//! Non-Negotiable Automotive Safety Invariants Enforced Here:
//! 1. Brake Override Veto: Driver mechanical/pedal brake depression suppresses
//!    advisory propulsion to 0 Nm unconditionally (Brake-Throttle Override).
//! 2. Torque Envelope & Slew Rate Clamping: Enforces per-motor maximum torque and
//!    bounds step changes (dT/dt) to prevent driveline shock and mechanical failure.
//! 3. Lateral Dynamics / Friction Circle Ceiling: Clamps left-to-right differential
//!    yaw moment to the tire-road adhesion limit (mu * Fz * track / 2).
//! 4. Vehicle & Drive State Gating: Blocks advisory torque unless the vehicle is in
//!    Drive state with torque enabled.
use helios_core::consts::G;
use helios_core::math::{abs, clamp};
use helios_core::{Corner, DriveMode, PerCorner, VehicleParams};

/// Advisory command originating from the non-ASIL Central HPC (Jetson Thor / Orin).
#[derive(Clone, Copy, Debug, Default)]
pub struct AdvisoryCommand {
    /// Advisory torque request per wheel, Nm (+ = forward propulsive).
    pub torque_request: PerCorner<f32>,
    /// Target road wheel angle, rad (positive = left).
    pub steer_road_request: f32,
    /// Target yaw rate, rad/s.
    pub yaw_rate_request: f32,
    /// Advisory engagement request flag from the HPC.
    pub enable: bool,
    /// Age of the advisory message packet in milliseconds.
    pub heartbeat_age_ms: u32,
    /// E2E CRC and rolling counter validity flag.
    pub crc_ok: bool,
}

/// Operational state of the supervisory firewall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FirewallState {
    /// Advisory control is inactive or not requested.
    #[default]
    Inactive = 0,
    /// Advisory demand passed all safety invariants without alteration.
    Pass = 1,
    /// Advisory demand exceeded safe bounds and was clamped to the boundary.
    Clamped = 2,
    /// Driver brake or pedal override vetoed advisory propulsion to zero.
    Vetoed = 3,
    /// Communication loss, CRC error, or unrecoverable invariant trip.
    Tripped = 4,
}

/// Bitflags representing specific invariant boundary violations detected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SupervisorViolations(pub u8);

impl SupervisorViolations {
    pub const NONE: Self = Self(0);
    pub const BRAKE_OVERRIDE: u8 = 1 << 0;
    pub const SLEW_RATE_CLAMPED: u8 = 1 << 1;
    pub const TORQUE_LIMIT_EXCEEDED: u8 = 1 << 2;
    pub const YAW_MOMENT_EXCEEDED: u8 = 1 << 3;
    pub const STATE_MISMATCH: u8 = 1 << 4;
    pub const HEARTBEAT_STALE: u8 = 1 << 5;
    pub const CRC_FAULT: u8 = 1 << 6;

    #[inline]
    pub const fn has(self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    #[inline]
    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }
}

/// Configuration parameters for the safety supervisor.
#[derive(Clone, Copy, Debug)]
pub struct SupervisorConfig {
    /// Maximum allowed packet age from the Central HPC before trip, ms.
    pub max_advisory_age_ms: u32,
    /// Maximum allowed torque slew rate per wheel, Nm/ms.
    pub max_slew_rate_nm_per_ms: f32,
    /// Brake pedal travel threshold above which propulsion is vetoed (0..1).
    pub brake_veto_pedal: f32,
    /// Safety margin factor applied to the tire friction circle (0..1).
    pub mu_margin: f32,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            max_advisory_age_ms: 50,
            max_slew_rate_nm_per_ms: 50.0,
            brake_veto_pedal: 0.02,
            mu_margin: 0.85,
        }
    }
}

/// Evaluated supervisor input snapshot for one control step (typically 1 ms).
#[derive(Clone, Copy, Debug)]
pub struct SupervisorInputs {
    pub advisory: AdvisoryCommand,
    pub speed_mps: f32,
    pub accel_pedal: f32,
    pub brake_pedal: f32,
    pub brake_pressed: bool,
    pub torque_enable: bool,
    pub drive_state_ok: bool,
    pub reverse: bool,
    pub motor_drive_limit: PerCorner<f32>,
    pub motor_regen_limit: PerCorner<f32>,
    pub p_discharge_limit: f32,
    pub yaw_rate: f32,
    pub steer_road: f32,
    pub mode: DriveMode,
}

/// Sanitized and validated outputs from the supervisor firewall.
#[derive(Clone, Copy, Debug, Default)]
pub struct SupervisorOutputs {
    /// Safe, clamped torque targets to dispatch to the physical VCU / inverters.
    pub sanitized_torque: PerCorner<f32>,
    /// Overall state of the safety firewall.
    pub state: FirewallState,
    /// Detected violation flags.
    pub violations: SupervisorViolations,
    /// Maximum torque difference (Nm) clamped across the corners.
    pub clamped_delta_max: f32,
}

/// ASIL D Safety Supervisor and Firewall engine.
#[derive(Clone, Copy, Debug)]
pub struct SafetySupervisor {
    pub p: VehicleParams,
    pub cfg: SupervisorConfig,
    dt: f32,
    prev_torque: PerCorner<f32>,
}

impl SafetySupervisor {
    pub fn new(p: VehicleParams, dt: f32, cfg: SupervisorConfig) -> Self {
        Self {
            p,
            cfg,
            dt,
            prev_torque: [0.0; 4],
        }
    }

    /// Step the supervisory firewall over one sample period (dt, typically 1 ms).
    pub fn step(&mut self, i: &SupervisorInputs) -> SupervisorOutputs {
        let mut out = SupervisorOutputs::default();
        let mut violations = SupervisorViolations::NONE;

        // Inactive if not requested by advisory source
        if !i.advisory.enable {
            self.prev_torque = [0.0; 4];
            out.state = FirewallState::Inactive;
            return out;
        }

        // --- Invariant 1: Communication Watchdog & Liveness ---
        if !i.advisory.crc_ok {
            violations.set(SupervisorViolations::CRC_FAULT);
            out.state = FirewallState::Tripped;
            out.violations = violations;
            self.prev_torque = [0.0; 4];
            return out;
        }

        if i.advisory.heartbeat_age_ms > self.cfg.max_advisory_age_ms {
            violations.set(SupervisorViolations::HEARTBEAT_STALE);
            out.state = FirewallState::Tripped;
            out.violations = violations;
            self.prev_torque = [0.0; 4];
            return out;
        }

        // --- Invariant 2: Vehicle & Drive State Gating ---
        if !i.drive_state_ok || !i.torque_enable {
            violations.set(SupervisorViolations::STATE_MISMATCH);
            out.state = FirewallState::Vetoed;
            out.violations = violations;
            self.prev_torque = [0.0; 4];
            return out;
        }

        // --- Invariant 3: Driver Brake Override Veto (Brake-Throttle Override) ---
        // If human driver presses the brake pedal, advisory propulsion is vetoed to 0.
        let driver_braking = i.brake_pressed || (i.brake_pedal > self.cfg.brake_veto_pedal);
        if driver_braking {
            violations.set(SupervisorViolations::BRAKE_OVERRIDE);
            out.state = FirewallState::Vetoed;
            out.violations = violations;
            self.prev_torque = [0.0; 4];
            return out;
        }

        // --- Invariant 4: Motor & Electrical Power Limits ---
        let v = abs(i.speed_mps);
        let radii = self.p.radii();
        let prof = i.mode.profile();
        let p_avail = (self.p.system_peak_power_w * prof.power_fraction).min(i.p_discharge_limit);
        let max_wheel_propulsion_force = p_avail / v.max(1.0);

        let mut clamped_t = [0.0f32; 4];
        let mut max_clamp_delta = 0.0f32;

        for c in Corner::ALL {
            let k = c.idx();
            let req = i.advisory.torque_request[k];
            let t_motor_max = self.p.motor.torque_limit(0.0).min(i.motor_drive_limit[k]);
            let t_power_max = (max_wheel_propulsion_force * radii[k] * 0.5).min(t_motor_max);

            // Forward propulsion bounds (in Reverse, signs invert)
            let (lo, hi) = if !i.reverse {
                (-i.motor_regen_limit[k].abs(), t_power_max)
            } else {
                (-t_power_max, i.motor_regen_limit[k].abs())
            };

            let bounded = clamp(req, lo, hi);
            if abs(req - bounded) > 1e-3 {
                violations.set(SupervisorViolations::TORQUE_LIMIT_EXCEEDED);
                max_clamp_delta = max_clamp_delta.max(abs(req - bounded));
            }
            clamped_t[k] = bounded;
        }

        // --- Invariant 5: Lateral Dynamics & Friction Circle Ceiling ---
        // Verify that the requested yaw moment does not exceed available adhesion.
        let fz_total = self.p.mass_kg * G;
        let mu = prof.mu_assumed;
        let track_m = 0.5 * (self.p.track_front_m + self.p.track_rear_m);
        let max_yaw_moment = self.cfg.mu_margin * mu * fz_total * (0.5 * track_m);

        // Calculate yaw moment produced by the requested torques
        let fx_fl = clamped_t[0] / radii[0];
        let fx_fr = clamped_t[1] / radii[1];
        let fx_rl = clamped_t[2] / radii[2];
        let fx_rr = clamped_t[3] / radii[3];
        let mz_front = (fx_fr - fx_fl) * (0.5 * self.p.track_front_m);
        let mz_rear = (fx_rr - fx_rl) * (0.5 * self.p.track_rear_m);
        let mz_dem = mz_front + mz_rear;

        if abs(mz_dem) > max_yaw_moment && max_yaw_moment > 0.0 {
            violations.set(SupervisorViolations::YAW_MOMENT_EXCEEDED);
            let scale = max_yaw_moment / abs(mz_dem);
            // Attenuate differential torque to remain inside friction circle
            let t_f_mean = 0.5 * (clamped_t[0] + clamped_t[1]);
            let t_f_diff = 0.5 * (clamped_t[1] - clamped_t[0]);
            clamped_t[0] = t_f_mean - t_f_diff * scale;
            clamped_t[1] = t_f_mean + t_f_diff * scale;

            let t_r_mean = 0.5 * (clamped_t[2] + clamped_t[3]);
            let t_r_diff = 0.5 * (clamped_t[3] - clamped_t[2]);
            clamped_t[2] = t_r_mean - t_r_diff * scale;
            clamped_t[3] = t_r_mean + t_r_diff * scale;
        }

        // --- Invariant 6: Slew Rate Limiter (dT/dt) ---
        let max_step = self.cfg.max_slew_rate_nm_per_ms * (self.dt * 1000.0).max(1.0);
        for k in 0..4 {
            let prev = self.prev_torque[k];
            let delta = clamped_t[k] - prev;
            if abs(delta) > max_step {
                violations.set(SupervisorViolations::SLEW_RATE_CLAMPED);
                let limited = prev + clamp(delta, -max_step, max_step);
                max_clamp_delta = max_clamp_delta.max(abs(clamped_t[k] - limited));
                clamped_t[k] = limited;
            }
        }

        self.prev_torque = clamped_t;
        out.sanitized_torque = clamped_t;
        out.clamped_delta_max = max_clamp_delta;
        out.violations = violations;
        out.state = if violations == SupervisorViolations::NONE {
            FirewallState::Pass
        } else {
            FirewallState::Clamped
        };

        out
    }

    /// Reset internal state (e.g. after a fault recovery).
    pub fn reset(&mut self) {
        self.prev_torque = [0.0; 4];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helios_core::params::HELIOS_BASELINE;

    fn make_test_supervisor() -> SafetySupervisor {
        SafetySupervisor::new(HELIOS_BASELINE, 0.001, SupervisorConfig::default())
    }

    fn nominal_inputs() -> SupervisorInputs {
        SupervisorInputs {
            advisory: AdvisoryCommand {
                torque_request: [100.0, 100.0, 200.0, 200.0],
                steer_road_request: 0.0,
                yaw_rate_request: 0.0,
                enable: true,
                heartbeat_age_ms: 10,
                crc_ok: true,
            },
            speed_mps: 20.0,
            accel_pedal: 0.3,
            brake_pedal: 0.0,
            brake_pressed: false,
            torque_enable: true,
            drive_state_ok: true,
            reverse: false,
            motor_drive_limit: [1000.0; 4],
            motor_regen_limit: [500.0; 4],
            p_discharge_limit: 1.0e6,
            yaw_rate: 0.0,
            steer_road: 0.0,
            mode: DriveMode::Road,
        }
    }

    #[test]
    fn nominal_advisory_passes_cleanly() {
        let mut sup = make_test_supervisor();
        let i = nominal_inputs();
        // Give slew rate a step to catch up from 0 to 100/200 Nm
        let mut out = sup.step(&i);
        while out.state == FirewallState::Clamped && out.violations.has(SupervisorViolations::SLEW_RATE_CLAMPED) {
            out = sup.step(&i);
        }
        assert_eq!(out.state, FirewallState::Pass);
        assert_eq!(out.violations, SupervisorViolations::NONE);
        assert_eq!(out.sanitized_torque, [100.0, 100.0, 200.0, 200.0]);
    }

    #[test]
    fn driver_brake_vetoes_advisory_propulsion() {
        let mut sup = make_test_supervisor();
        let mut i = nominal_inputs();
        i.brake_pressed = true;
        let out = sup.step(&i);
        assert_eq!(out.state, FirewallState::Vetoed);
        assert!(out.violations.has(SupervisorViolations::BRAKE_OVERRIDE));
        assert_eq!(out.sanitized_torque, [0.0; 4]);
    }

    #[test]
    fn stale_heartbeat_trips_firewall() {
        let mut sup = make_test_supervisor();
        let mut i = nominal_inputs();
        i.advisory.heartbeat_age_ms = 100; // > 50 ms limit
        let out = sup.step(&i);
        assert_eq!(out.state, FirewallState::Tripped);
        assert!(out.violations.has(SupervisorViolations::HEARTBEAT_STALE));
        assert_eq!(out.sanitized_torque, [0.0; 4]);
    }

    #[test]
    fn crc_failure_trips_firewall() {
        let mut sup = make_test_supervisor();
        let mut i = nominal_inputs();
        i.advisory.crc_ok = false;
        let out = sup.step(&i);
        assert_eq!(out.state, FirewallState::Tripped);
        assert!(out.violations.has(SupervisorViolations::CRC_FAULT));
        assert_eq!(out.sanitized_torque, [0.0; 4]);
    }

    #[test]
    fn unphysical_torque_request_is_clamped() {
        let mut sup = make_test_supervisor();
        let mut i = nominal_inputs();
        i.advisory.torque_request = [10_000.0; 4]; // Far exceeds motor envelope
        let out = sup.step(&i);
        assert!(out.violations.has(SupervisorViolations::TORQUE_LIMIT_EXCEEDED));
        assert!(out.clamped_delta_max > 0.0);
    }

    #[test]
    fn unphysical_yaw_moment_is_clamped_to_friction_circle() {
        let mut cfg = SupervisorConfig::default();
        cfg.mu_margin = 0.2; // Strict margin or low-adhesion surface test
        let mut sup = SafetySupervisor::new(HELIOS_BASELINE, 0.001, cfg);
        let mut i = nominal_inputs();
        // Request massive left/right split to spin vehicle
        i.advisory.torque_request = [-500.0, 800.0, -500.0, 800.0];
        let out = sup.step(&i);
        assert!(out.violations.has(SupervisorViolations::YAW_MOMENT_EXCEEDED));
    }
}
