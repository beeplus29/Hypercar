//! HELIOS energy domain.
//!
//! * [`Battery`]: lumped-cell 2-RC equivalent circuit, exact ZOH discretisation,
//!   Arrhenius resistance, lumped pack thermal mass coupled to a coolant loop.
//! * [`state_of_power`]: discharge/charge current and power limits over a
//!   horizon (HPPC-style), honouring cell voltage window, C-rate and thermal derates.
//! * [`SocEkf`]: 3-state extended Kalman filter (SOC, V1, V2) on terminal voltage.
//! * [`MotorThermal`]: winding thermal node and torque derate.
//!
//! Sign convention: current > 0 = discharge.
#![no_std]
#![forbid(unsafe_code)]

use helios_core::math::{clamp, exp, interp1};
use helios_core::params::{BatterySpec, MotorSpec, OCV_SOC, OCV_V, cell_ocv};

/// dOCV/dSOC by central difference on the OCV table, V per unit SOC.
pub fn docv_dsoc(soc: f32) -> f32 {
    let h = 0.01;
    let a = clamp(soc - h, 0.0, 1.0);
    let b = clamp(soc + h, 0.0, 1.0);
    (cell_ocv(b) - cell_ocv(a)) / (b - a).max(1e-6)
}

/// Kahan-Babuska compensated addition: returns (sum, new compensation).
#[inline]
pub fn kahan_add(sum: f32, comp: f32, x: f32) -> (f32, f32) {
    let y = x - comp;
    let t = sum + y;
    (t, (t - sum) - y)
}

/// Inverse OCV lookup (monotonic table).
pub fn soc_from_ocv(v: f32) -> f32 {
    interp1(&OCV_V, &OCV_SOC, v)
}

#[derive(Clone, Copy, Debug)]
pub struct CoolantLoop {
    /// Coolant + plate thermal mass, J/K.
    pub c_j_k: f32,
    /// Max chiller heat rejection, W (refrigerant loop via 800 V e-compressor).
    pub chiller_max_w: f32,
    /// Chiller effectiveness * m_dot * cp, W/K.
    pub chiller_ua_w_k: f32,
    /// Front radiator UA at zero speed and per m/s of ram air, W/K.
    pub radiator_ua0_w_k: f32,
    pub radiator_ua_per_mps: f32,
    pub chiller_setpoint_c: f32,
}

impl CoolantLoop {
    pub const HELIOS: CoolantLoop = CoolantLoop {
        c_j_k: 25.0e3, // ~6 L glycol-water (3.6 kJ/kgK) + Al cold plates
        chiller_max_w: 12.0e3,
        chiller_ua_w_k: 1500.0,
        radiator_ua0_w_k: 150.0,
        radiator_ua_per_mps: 25.0,
        chiller_setpoint_c: 22.0,
    };
}

#[derive(Clone, Copy, Debug)]
pub struct BatteryState {
    pub soc: f32,
    /// Kahan compensation term for SOC integration. At 1 ms steps a 135 A cell-group
    /// current changes SOC by ~1.9e-7 per step, about 3 ULP of f32 at SOC 0.9: plain
    /// summation drifts ~0.7 % SOC in 6 minutes (caught by `coulomb_counting_matches_capacity`).
    pub soc_comp: f32,
    pub v1: f32,
    pub v2: f32,
    pub t_cell_c: f32,
    pub t_coolant_c: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BatteryOutputs {
    pub v_terminal: f32,
    pub v_ocv: f32,
    pub i_pack: f32,
    pub p_terminal: f32,
    pub q_gen_w: f32,
    pub q_chiller_w: f32,
    pub q_radiator_w: f32,
    pub r0_pack: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Battery {
    pub spec: BatterySpec,
    pub cool: CoolantLoop,
    pub s: BatteryState,
    pub chiller_on: bool,
}

impl Battery {
    pub fn new(spec: BatterySpec, cool: CoolantLoop, soc: f32, t_c: f32) -> Self {
        Self { spec, cool, s: BatteryState { soc, soc_comp: 0.0, v1: 0.0, v2: 0.0, t_cell_c: t_c, t_coolant_c: t_c }, chiller_on: true }
    }

    /// Terminal voltage at pack current `i` without advancing state.
    pub fn terminal_voltage(&self, i_pack: f32) -> f32 {
        let b = &self.spec;
        let i_cell = i_pack / b.parallel as f32;
        let f = b.r_temp_factor(self.s.t_cell_c);
        let v_cell = cell_ocv(self.s.soc) - self.s.v1 - self.s.v2 - i_cell * b.cell_r0_ohm * f;
        v_cell * b.series as f32
    }

    /// Solve pack current for a terminal power demand P = V(I) I (quadratic in I).
    /// Returns None if P exceeds the maximum transferable power.
    pub fn current_for_power(&self, p: f32) -> Option<f32> {
        let b = &self.spec;
        let ns = b.series as f32;
        let np = b.parallel as f32;
        let e = (cell_ocv(self.s.soc) - self.s.v1 - self.s.v2) * ns;
        let r = b.cell_r0_ohm * b.r_temp_factor(self.s.t_cell_c) * ns / np;
        // P = e I - r I^2  =>  r I^2 - e I + P = 0
        let disc = e * e - 4.0 * r * p;
        if disc < 0.0 {
            return None;
        }
        Some((e - libm::sqrtf(disc)) / (2.0 * r))
    }

    /// Advance by dt with pack current `i_pack` (A), ambient temperature and vehicle speed
    /// (for radiator ram air).
    pub fn step(&mut self, i_pack: f32, dt: f32, t_amb_c: f32, speed: f32) -> BatteryOutputs {
        let b = self.spec;
        let np = b.parallel as f32;
        let ns = b.series as f32;
        let i_cell = i_pack / np;
        let f = b.r_temp_factor(self.s.t_cell_c);
        let (r0, r1, r2) = (b.cell_r0_ohm * f, b.cell_r1_ohm * f, b.cell_r2_ohm * f);
        let ocv = cell_ocv(self.s.soc);
        let v_cell = ocv - self.s.v1 - self.s.v2 - i_cell * r0;
        // Irreversible heat per cell (entropic term neglected, < 5 % at high C-rate).
        let q_cell = i_cell * i_cell * r0 + self.s.v1 * self.s.v1 / r1 + self.s.v2 * self.s.v2 / r2;
        let q_gen = q_cell * b.n_cells() as f32;
        // Exact ZOH for the RC branches.
        let a1 = exp(-dt / b.cell_tau1_s);
        let a2 = exp(-dt / b.cell_tau2_s);
        self.s.v1 = a1 * self.s.v1 + r1 * (1.0 - a1) * i_cell;
        self.s.v2 = a2 * self.s.v2 + r2 * (1.0 - a2) * i_cell;
        let (soc, comp) = kahan_add(self.s.soc, self.s.soc_comp, -i_cell * dt / (3600.0 * b.cell_capacity_ah));
        self.s.soc = clamp(soc, 0.0, 1.0);
        self.s.soc_comp = if soc == self.s.soc { comp } else { 0.0 };
        // Thermal: cell node <-> coolant node <-> chiller / radiator.
        let q_to_cool = b.pack_ua_w_k * (self.s.t_cell_c - self.s.t_coolant_c);
        let q_chill = if self.chiller_on && self.s.t_coolant_c > self.cool.chiller_setpoint_c {
            (self.cool.chiller_ua_w_k * (self.s.t_coolant_c - self.cool.chiller_setpoint_c)).min(self.cool.chiller_max_w)
        } else {
            0.0
        };
        let ua_rad = self.cool.radiator_ua0_w_k + self.cool.radiator_ua_per_mps * speed.max(0.0);
        let q_rad = ua_rad * (self.s.t_coolant_c - t_amb_c);
        self.s.t_cell_c += dt * (q_gen - q_to_cool) / b.thermal_mass_j_k();
        self.s.t_coolant_c += dt * (q_to_cool - q_chill - q_rad) / self.cool.c_j_k;
        BatteryOutputs {
            v_terminal: v_cell * ns,
            v_ocv: ocv * ns,
            i_pack,
            p_terminal: v_cell * ns * i_pack,
            q_gen_w: q_gen,
            q_chiller_w: q_chill,
            q_radiator_w: q_rad,
            r0_pack: r0 * ns / np,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerLimits {
    pub i_discharge_a: f32,
    pub i_charge_a: f32,
    pub p_discharge_w: f32,
    pub p_charge_w: f32,
    pub thermal_derate: bool,
}

/// State of power over horizon `horizon_s` (typically 10 s for the VCU, 2 s peak).
///
/// The terminal voltage after holding constant current I for dt is (per cell)
///   V(dt) = OCV(SOC) - k_ocv I dt/(3600 Q) - V1 a1 - V2 a2 - I (R0 + R1 (1-a1) + R2 (1-a2))
/// with a_j = exp(-dt/tau_j). Setting V(dt) = V_min (discharge) or V_max (charge) gives
/// the current limit in closed form. Power uses the terminal voltage at that current.
pub fn state_of_power(spec: &BatterySpec, s: &BatteryState, horizon_s: f32) -> PowerLimits {
    let f = spec.r_temp_factor(s.t_cell_c);
    let (r0, r1, r2) = (spec.cell_r0_ohm * f, spec.cell_r1_ohm * f, spec.cell_r2_ohm * f);
    let a1 = exp(-horizon_s / spec.cell_tau1_s);
    let a2 = exp(-horizon_s / spec.cell_tau2_s);
    let k_ocv = docv_dsoc(s.soc) * horizon_s / (3600.0 * spec.cell_capacity_ah);
    let r_eff = r0 + r1 * (1.0 - a1) + r2 * (1.0 - a2) + k_ocv;
    let e = cell_ocv(s.soc) - s.v1 * a1 - s.v2 * a2;
    let i_dis_v = ((e - spec.cell_v_min) / r_eff).max(0.0);
    let i_chg_v = ((spec.cell_v_max - e) / r_eff).max(0.0);
    // Thermal derates: linear from derate start to max cell temperature; charge derate
    // below 15 degC to zero at 0 degC (lithium-plating guard for an NMC/graphite baseline).
    let t = s.t_cell_c;
    let hot = clamp((spec.t_max_c - t) / (spec.t_max_c - spec.t_derate_start_c), 0.0, 1.0);
    let cold_chg = clamp(t / 15.0, 0.0, 1.0);
    let np = spec.parallel as f32;
    let ns = spec.series as f32;
    let i_dis_cell = i_dis_v.min(spec.max_discharge_c * spec.cell_capacity_ah) * hot;
    let i_chg_cell = i_chg_v.min(spec.max_charge_c * spec.cell_capacity_ah) * hot * cold_chg;
    let v_dis = (e - i_dis_cell * r_eff).max(spec.cell_v_min);
    let v_chg = (e + i_chg_cell * r_eff).min(spec.cell_v_max);
    PowerLimits {
        i_discharge_a: i_dis_cell * np,
        i_charge_a: i_chg_cell * np,
        p_discharge_w: i_dis_cell * np * v_dis * ns,
        p_charge_w: i_chg_cell * np * v_chg * ns,
        thermal_derate: hot < 1.0 || cold_chg < 1.0,
    }
}

/// Extended Kalman filter for SOC on a lumped cell (state x = [SOC, V1, V2]).
#[derive(Clone, Copy, Debug)]
pub struct SocEkf {
    pub x: [f32; 3],
    pub p: [[f32; 3]; 3],
    pub q: [f32; 3],
    pub r: f32,
    soc_comp: f32,
    spec: BatterySpec,
}

impl SocEkf {
    pub fn new(spec: BatterySpec, soc0: f32) -> Self {
        Self {
            x: [soc0, 0.0, 0.0],
            p: [[0.05, 0.0, 0.0], [0.0, 1e-4, 0.0], [0.0, 0.0, 1e-4]],
            q: [1e-10, 1e-8, 1e-8],
            r: 2.5e-5, // (5 mV)^2 cell-voltage measurement noise
            soc_comp: 0.0,
            spec,
        }
    }
    pub fn soc(&self) -> f32 {
        self.x[0]
    }
    /// Predict with pack current and correct with measured pack terminal voltage.
    pub fn step(&mut self, i_pack: f32, v_pack_meas: f32, t_cell_c: f32, dt: f32) {
        let b = &self.spec;
        let i = i_pack / b.parallel as f32;
        let f = b.r_temp_factor(t_cell_c);
        let (r0, r1, r2) = (b.cell_r0_ohm * f, b.cell_r1_ohm * f, b.cell_r2_ohm * f);
        let a1 = exp(-dt / b.cell_tau1_s);
        let a2 = exp(-dt / b.cell_tau2_s);
        // Predict: x+ = A x + B i, A = diag(1, a1, a2)
        let (soc, comp) = kahan_add(self.x[0], self.soc_comp, -i * dt / (3600.0 * b.cell_capacity_ah));
        self.x[0] = soc;
        self.soc_comp = comp;
        self.x[1] = a1 * self.x[1] + r1 * (1.0 - a1) * i;
        self.x[2] = a2 * self.x[2] + r2 * (1.0 - a2) * i;
        let a = [1.0, a1, a2];
        for r in 0..3 {
            for c in 0..3 {
                self.p[r][c] *= a[r] * a[c];
            }
            self.p[r][r] += self.q[r];
        }
        // Correct: y = OCV(SOC) - V1 - V2 - i R0; H = [dOCV/dSOC, -1, -1]
        let v_meas = v_pack_meas / b.series as f32;
        let y_hat = cell_ocv(self.x[0]) - self.x[1] - self.x[2] - i * r0;
        let h = [docv_dsoc(self.x[0]), -1.0, -1.0];
        let mut ph = [0.0f32; 3];
        for r in 0..3 {
            for c in 0..3 {
                ph[r] += self.p[r][c] * h[c];
            }
        }
        let s = h[0] * ph[0] + h[1] * ph[1] + h[2] * ph[2] + self.r;
        let k = [ph[0] / s, ph[1] / s, ph[2] / s];
        let innov = v_meas - y_hat;
        for r in 0..3 {
            self.x[r] += k[r] * innov;
        }
        // Joseph-free update P = (I - K H) P, then symmetrise.
        let mut np = self.p;
        for r in 0..3 {
            for c in 0..3 {
                np[r][c] = self.p[r][c] - k[r] * ph[c];
            }
        }
        for r in 0..3 {
            for c in 0..3 {
                self.p[r][c] = 0.5 * (np[r][c] + np[c][r]);
            }
        }
        self.x[0] = clamp(self.x[0], 0.0, 1.0);
    }
}

/// Motor winding thermal node with torque derate.
#[derive(Clone, Copy, Debug)]
pub struct MotorThermal {
    pub t_winding_c: f32,
}

impl MotorThermal {
    pub fn step(&mut self, m: &MotorSpec, torque: f32, omega: f32, t_coolant_c: f32, dt: f32) -> f32 {
        let p_loss = m.loss(torque, omega);
        self.t_winding_c += dt * (p_loss - m.winding_ua_w_k * (self.t_winding_c - t_coolant_c)) / m.winding_c_j_k;
        p_loss
    }
    /// Torque derate factor (1 below derate start, 0 at derate end).
    pub fn derate(&self, m: &MotorSpec) -> f32 {
        clamp((m.derate_end_c - self.t_winding_c) / (m.derate_end_c - m.derate_start_c), 0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helios_core::params::HELIOS_BASELINE;

    fn bat() -> Battery {
        Battery::new(HELIOS_BASELINE.battery, CoolantLoop::HELIOS, 0.9, 25.0)
    }

    #[test]
    fn coulomb_counting_matches_capacity() {
        let mut b = bat();
        // 135 Ah pack, discharge 135 A for 360 s => 13.5 Ah => 10 % SOC
        for _ in 0..360_000 {
            b.step(135.0, 0.001, 25.0, 0.0);
        }
        assert!((b.s.soc - 0.8).abs() < 1e-3, "{}", b.s.soc);
    }

    #[test]
    fn rc_branch_reaches_ir_steady_state() {
        let mut b = bat();
        let i = 300.0;
        for _ in 0..2000 {
            b.step(i, 0.5, 25.0, 0.0);
        }
        // After 1000 s (10 tau2): V1 -> R1(T) I_cell, V2 -> R2(T) I_cell. The pack self-heats
        // during the run, so the Arrhenius factor at the final temperature applies.
        let ic = i / 3.0;
        let f = b.spec.r_temp_factor(b.s.t_cell_c);
        assert!(b.s.t_cell_c > 25.0);
        assert!((b.s.v1 - 0.000_25 * f * ic).abs() < 2e-5, "{} vs {}", b.s.v1, 0.000_25 * f * ic);
        assert!((b.s.v2 - 0.000_20 * f * ic).abs() < 2e-4, "{} vs {}", b.s.v2, 0.000_20 * f * ic);
    }

    #[test]
    fn current_for_power_inverts_terminal_power() {
        let b = bat();
        let i = b.current_for_power(1.2e6).expect("feasible");
        let p = b.terminal_voltage(i) * i;
        assert!((p - 1.2e6).abs() / 1.2e6 < 1e-3, "{p}");
    }

    #[test]
    fn sop_is_consistent_with_voltage_limit() {
        let b = bat();
        let l = state_of_power(&b.spec, &b.s, 10.0);
        // Simulate holding the discharge limit for the horizon: terminal must end at >= V_min.
        let mut sim = b;
        let mut v_end = 0.0;
        for _ in 0..10_000 {
            v_end = sim.step(l.i_discharge_a, 0.001, 25.0, 0.0).v_terminal;
        }
        let v_min = b.spec.v_min_pack();
        assert!(v_end >= v_min - 2.0, "v_end {v_end} below {v_min}");
        assert!(l.p_discharge_w > 1.4e6, "pack SOP {} W", l.p_discharge_w);
    }

    #[test]
    fn hot_pack_derates_to_zero_at_t_max() {
        let mut s = bat().s;
        s.t_cell_c = 60.0;
        let l = state_of_power(&HELIOS_BASELINE.battery, &s, 10.0);
        assert!(l.p_discharge_w < 1.0 && l.thermal_derate);
        s.t_cell_c = 55.0;
        let half = state_of_power(&HELIOS_BASELINE.battery, &s, 10.0);
        s.t_cell_c = 30.0;
        let full = state_of_power(&HELIOS_BASELINE.battery, &s, 10.0);
        assert!((half.i_discharge_a / full.i_discharge_a - 0.5).abs() < 0.05);
    }

    #[test]
    fn ekf_converges_from_wrong_initial_soc() {
        let mut b = bat();
        b.s.soc = 0.6;
        let mut ekf = SocEkf::new(b.spec, 0.85);
        for k in 0..600_000 {
            // 0.1 Hz square-wave 200 A / -100 A drive cycle, 10 ms steps, 100 min
            let i = if (k / 1000) % 2 == 0 { 200.0 } else { -100.0 };
            let o = b.step(i, 0.01, 25.0, 0.0);
            ekf.step(i, o.v_terminal, b.s.t_cell_c, 0.01);
        }
        assert!((ekf.soc() - b.s.soc).abs() < 0.01, "ekf {} true {}", ekf.soc(), b.s.soc);
    }

    #[test]
    fn motor_winding_steady_state_temperature() {
        let m = HELIOS_BASELINE.motor;
        let mut th = MotorThermal { t_winding_c: 40.0 };
        let (t, w) = (1500.0, 100.0);
        for _ in 0..200_000 {
            th.step(&m, t, w, 40.0, 0.01);
        }
        let expected = 40.0 + m.loss(t, w) / m.winding_ua_w_k;
        assert!((th.t_winding_c - expected).abs() < 0.1, "{} vs {}", th.t_winding_c, expected);
    }
}
