//! Planar twin-track vehicle plant (f64).
//!
//! States (27): X, Y, psi, vx, vy, r | omega[4] | kappa'[4], alpha'[4] (first-order
//! relaxation-length transient slip) | motor torque[4] (current-loop lag) |
//! friction brake torque[4] (brake-by-wire lag) | aero CdA, ClA, balance (wing
//! actuator lag). Vertical loads are quasi-static from the previous step's
//! accelerations (no heave/pitch/roll DOF). Tyres: Magic Formula pure slip with
//! load-sensitive friction, combined by friction-ellipse capping.
//! Integrator: classical RK4.

use helios_core::params::VehicleParams;
use helios_core::{AeroMode, Corner};

pub const V_FLOOR: f64 = 0.5;

#[derive(Clone, Copy, Debug, Default)]
pub struct PlantState {
    pub x: f64,
    pub y: f64,
    pub psi: f64,
    pub vx: f64,
    pub vy: f64,
    pub r: f64,
    pub omega: [f64; 4],
    pub kappa: [f64; 4],
    pub alpha: [f64; 4],
    pub t_motor: [f64; 4],
    pub t_brake: [f64; 4],
    pub cd_a: f64,
    pub cl_a: f64,
    pub bal: f64,
}

impl PlantState {
    fn axpy(&self, k: &PlantState, h: f64) -> PlantState {
        let mut o = *self;
        o.x += h * k.x;
        o.y += h * k.y;
        o.psi += h * k.psi;
        o.vx += h * k.vx;
        o.vy += h * k.vy;
        o.r += h * k.r;
        for i in 0..4 {
            o.omega[i] += h * k.omega[i];
            o.kappa[i] += h * k.kappa[i];
            o.alpha[i] += h * k.alpha[i];
            o.t_motor[i] += h * k.t_motor[i];
            o.t_brake[i] += h * k.t_brake[i];
        }
        o.cd_a += h * k.cd_a;
        o.cl_a += h * k.cl_a;
        o.bal += h * k.bal;
        o
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlantInputs {
    pub motor_cmd: [f64; 4],
    pub brake_cmd: [f64; 4],
    pub delta_road: f64,
    pub aero_mode: AeroMode,
    /// Motor hard-failed (open phases): torque decays to zero regardless of command.
    pub motor_dead: [bool; 4],
    /// Road friction scale (1 = dry track, 0.6 = wet).
    pub mu_scale: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlantAux {
    pub ax: f64,
    pub ay: f64,
    pub fz: [f64; 4],
    pub fx: [f64; 4],
    pub fy: [f64; 4],
    pub drag: f64,
    pub downforce: f64,
    pub slip_ss: [f64; 4],
}

#[derive(Clone, Debug)]
pub struct Plant {
    pub p: VehicleParams,
    pub s: PlantState,
    pub aux: PlantAux,
    pub motor_tau: f64,
    pub brake_tau: f64,
}

fn mf(b: f64, c: f64, d: f64, e: f64, x: f64) -> f64 {
    let bx = b * x;
    d * (c * (bx - e * (bx - bx.atan())).atan()).sin()
}

impl Plant {
    pub fn new(p: VehicleParams) -> Self {
        let a = p.aero.high_downforce;
        Self {
            p,
            s: PlantState { cd_a: a.cd_a as f64, cl_a: a.cl_a as f64, bal: a.balance_front as f64, ..Default::default() },
            aux: PlantAux::default(),
            motor_tau: p.motor.torque_tau_s as f64,
            brake_tau: 0.030,
        }
    }

    fn fz(&self, s: &PlantState, ax: f64, ay: f64) -> [f64; 4] {
        let p = &self.p;
        let (m, l, h) = (p.mass_kg as f64, p.wheelbase_m as f64, p.cg_height_m as f64);
        let g = helios_core::consts::G as f64;
        let rho = helios_core::consts::RHO_AIR_ISA as f64;
        let df = 0.5 * rho * s.cl_a * s.vx * s.vx;
        let a = p.a() as f64;
        let b = l - a;
        let d_long = m * ax * h / l;
        let rf = p.roll_stiffness_front as f64;
        let d_lat_f = m * ay * h * rf / p.track_front_m as f64;
        let d_lat_r = m * ay * h * (1.0 - rf) / p.track_rear_m as f64;
        let mut fz = [0.0; 4];
        for c in Corner::ALL {
            let st = m * g * if c.is_front() { b / l } else { a / l } / 2.0;
            let aero = if c.is_front() { df * s.bal / 2.0 } else { df * (1.0 - s.bal) / 2.0 };
            let long = if c.is_front() { -d_long / 2.0 } else { d_long / 2.0 };
            let lat = if c.is_front() { d_lat_f } else { d_lat_r };
            let lat = if c.is_left() { -lat } else { lat };
            fz[c.idx()] = (st + aero + long + lat).max(0.0);
        }
        fz
    }

    /// State derivative; also returns auxiliary quantities.
    fn deriv(&self, s: &PlantState, u: &PlantInputs, ax_prev: f64, ay_prev: f64) -> (PlantState, PlantAux) {
        let p = &self.p;
        let m = p.mass_kg as f64;
        let iz = p.yaw_inertia_kgm2 as f64;
        let iw = p.wheel_inertia_kgm2 as f64;
        let rho = helios_core::consts::RHO_AIR_ISA as f64;
        let mf_p = p.tyre_mf;
        let fz = self.fz(s, ax_prev, ay_prev);
        let deltas = p.ackermann(u.delta_road as f32).map(|d| d as f64);
        let mut d = PlantState::default();
        let mut aux = PlantAux { fz, ..Default::default() };
        let (mut fxb, mut fyb, mut mz) = (0.0, 0.0, 0.0);
        for c in Corner::ALL {
            let k = c.idx();
            let (xc, yc) = p.corner_xy(c);
            let (xc, yc) = (xc as f64, yc as f64);
            let rr = p.rolling_radius(c) as f64;
            let vxh = s.vx - s.r * yc;
            let vyh = s.vy + s.r * xc;
            let (sd, cd) = deltas[k].sin_cos();
            let vcx = vxh * cd + vyh * sd;
            let vcy = -vxh * sd + vyh * cd;
            let den = vcx.abs().max(V_FLOOR);
            let kappa_ss = (s.omega[k] * rr - vcx) / den;
            let alpha_ss = vcy.atan2(den);
            aux.slip_ss[k] = kappa_ss;
            let rate = (vcx.abs() + V_FLOOR) / mf_p.sigma_x as f64;
            let rate_y = (vcx.abs() + V_FLOOR) / mf_p.sigma_y as f64;
            d.kappa[k] = rate * (kappa_ss - s.kappa[k]);
            d.alpha[k] = rate_y * (alpha_ss - s.alpha[k]);
            let mu = mf_p.mu(fz[k] as f32) as f64 * u.mu_scale;
            let dmax = mu * fz[k];
            let fx0 = mf(mf_p.bx as f64, mf_p.cx as f64, dmax, mf_p.ex as f64, s.kappa[k]);
            let fy0 = -mf(mf_p.by as f64, mf_p.cy as f64, dmax, mf_p.ey as f64, s.alpha[k]);
            let n = if dmax > 1.0 { ((fx0 / dmax).powi(2) + (fy0 / dmax).powi(2)).sqrt() } else { 0.0 };
            let sc = if n > 1.0 { 1.0 / n } else { 1.0 };
            // Rolling resistance is a moment on the wheel (pressure-centre offset), not a
            // contact force: modelling it as a force would be cancelled by the spin equation.
            let roll = mf_p.crr as f64 * fz[k] * (s.omega[k] / 0.5).tanh();
            let fx = fx0 * sc;
            let fy = fy0 * sc;
            aux.fx[k] = fx;
            aux.fy[k] = fy;
            // Body-frame components
            let fbx = fx * cd - fy * sd;
            let fby = fx * sd + fy * cd;
            fxb += fbx;
            fyb += fby;
            mz += xc * fby - yc * fbx;
            // Wheel spin: friction brake opposes rotation (smoothed sign)
            let tb = s.t_brake[k].max(0.0) * (s.omega[k] / 0.2).tanh();
            d.omega[k] = (s.t_motor[k] - tb - fx * rr - roll * rr) / iw;
            // Actuators
            let env = p.motor.torque_limit(s.omega[k] as f32) as f64;
            let cmd = if u.motor_dead[k] { 0.0 } else { u.motor_cmd[k].clamp(-env, env) };
            d.t_motor[k] = (cmd - s.t_motor[k]) / self.motor_tau;
            d.t_brake[k] = (u.brake_cmd[k].max(0.0) - s.t_brake[k]) / self.brake_tau;
        }
        let drag = 0.5 * rho * s.cd_a * s.vx * s.vx.abs();
        aux.drag = drag;
        aux.downforce = 0.5 * rho * s.cl_a * s.vx * s.vx;
        fxb -= drag;
        d.vx = fxb / m + s.r * s.vy;
        d.vy = fyb / m - s.r * s.vx;
        d.r = mz / iz;
        let (sp, cp) = s.psi.sin_cos();
        d.x = s.vx * cp - s.vy * sp;
        d.y = s.vx * sp + s.vy * cp;
        d.psi = s.r;
        aux.ax = fxb / m;
        aux.ay = fyb / m;
        let tgt = p.aero.point(u.aero_mode);
        let tau_a = p.aero.transition_time_s as f64 / 3.0;
        d.cd_a = (tgt.cd_a as f64 - s.cd_a) / tau_a;
        d.cl_a = (tgt.cl_a as f64 - s.cl_a) / tau_a;
        d.bal = (tgt.balance_front as f64 - s.bal) / tau_a;
        (d, aux)
    }

    pub fn step(&mut self, u: &PlantInputs, h: f64) {
        let (ax, ay) = (self.aux.ax, self.aux.ay);
        let s0 = self.s;
        let (k1, aux) = self.deriv(&s0, u, ax, ay);
        let (k2, _) = self.deriv(&s0.axpy(&k1, h / 2.0), u, ax, ay);
        let (k3, _) = self.deriv(&s0.axpy(&k2, h / 2.0), u, ax, ay);
        let (k4, _) = self.deriv(&s0.axpy(&k3, h), u, ax, ay);
        let mut s = s0;
        let w = |a: f64, b: f64, c: f64, d: f64| (a + 2.0 * b + 2.0 * c + d) / 6.0;
        s.x += h * w(k1.x, k2.x, k3.x, k4.x);
        s.y += h * w(k1.y, k2.y, k3.y, k4.y);
        s.psi += h * w(k1.psi, k2.psi, k3.psi, k4.psi);
        s.vx += h * w(k1.vx, k2.vx, k3.vx, k4.vx);
        s.vy += h * w(k1.vy, k2.vy, k3.vy, k4.vy);
        s.r += h * w(k1.r, k2.r, k3.r, k4.r);
        for i in 0..4 {
            s.omega[i] += h * w(k1.omega[i], k2.omega[i], k3.omega[i], k4.omega[i]);
            s.kappa[i] += h * w(k1.kappa[i], k2.kappa[i], k3.kappa[i], k4.kappa[i]);
            s.alpha[i] += h * w(k1.alpha[i], k2.alpha[i], k3.alpha[i], k4.alpha[i]);
            s.t_motor[i] += h * w(k1.t_motor[i], k2.t_motor[i], k3.t_motor[i], k4.t_motor[i]);
            s.t_brake[i] += h * w(k1.t_brake[i], k2.t_brake[i], k3.t_brake[i], k4.t_brake[i]);
        }
        s.cd_a += h * w(k1.cd_a, k2.cd_a, k3.cd_a, k4.cd_a);
        s.cl_a += h * w(k1.cl_a, k2.cl_a, k3.cl_a, k4.cl_a);
        s.bal += h * w(k1.bal, k2.bal, k3.bal, k4.bal);
        // Holding brake at standstill: kill numerical creep when braked and nearly stopped.
        for i in 0..4 {
            if s.t_brake[i] > 50.0 && s.omega[i].abs() < 0.05 && s.t_motor[i].abs() < s.t_brake[i] {
                s.omega[i] = 0.0;
            }
        }
        if s.vx.abs() < 0.02 && s.omega.iter().all(|w| *w == 0.0) {
            s.vx = 0.0;
            s.vy = 0.0;
            s.r = 0.0;
        }
        self.s = s;
        self.aux = aux;
    }

    pub fn speed(&self) -> f64 {
        (self.s.vx * self.s.vx + self.s.vy * self.s.vy).sqrt()
    }

    /// Electrical power at the DC link for the current motor torques (W, + = discharge).
    pub fn dc_power(&self) -> f64 {
        let p = &self.p;
        let eta = p.inverter_eta as f64;
        let mut pe = 0.0;
        for k in 0..4 {
            let t = self.s.t_motor[k];
            let w = self.s.omega[k];
            let e = t * w + p.motor.loss(t as f32, w as f32) as f64;
            pe += if e >= 0.0 { e / eta } else { e * eta };
        }
        pe
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helios_core::params::HELIOS_BASELINE;

    #[test]
    fn coastdown_matches_drag_and_rolling_resistance() {
        // Closed form: v' = -(D + Crr (m g + L)) / (m + sum Iw / r^2)
        let p = HELIOS_BASELINE;
        let mut pl = Plant::new(p);
        let v0 = 50.0;
        pl.s.vx = v0;
        for c in Corner::ALL {
            pl.s.omega[c.idx()] = v0 / p.rolling_radius(c) as f64;
        }
        let u = PlantInputs { aero_mode: AeroMode::HighDownforce, mu_scale: 1.0, ..Default::default() };
        for _ in 0..2000 {
            pl.step(&u, 0.00025);
        }
        let v = pl.s.vx;
        let m = p.mass_kg as f64;
        let drag = 0.5 * 1.225 * 0.82 * v * v;
        let df = 0.5 * 1.225 * 2.40 * v * v;
        let roll = 0.012 * (m * 9.80665 + df);
        let m_eff: f64 = m + Corner::ALL.iter().map(|c| 2.9 / (p.rolling_radius(*c) as f64).powi(2)).sum::<f64>();
        let a_expected = -(drag + roll) / m_eff;
        assert!((pl.aux.ax - a_expected).abs() < 0.01, "ax {} expected {}", pl.aux.ax, a_expected);
    }

    #[test]
    fn steady_cornering_yaw_rate_matches_kinematics_at_low_speed() {
        let p = HELIOS_BASELINE;
        let mut pl = Plant::new(p);
        let v = 5.0;
        let delta = 0.1;
        pl.s.vx = v;
        let mut u = PlantInputs { delta_road: delta, aero_mode: AeroMode::HighDownforce, mu_scale: 1.0, ..Default::default() };
        for _ in 0..40_000 {
            // simple speed hold on the rear motors
            let e = v - pl.s.vx;
            let t = 2000.0 * e;
            u.motor_cmd = [0.0, 0.0, t, t];
            pl.step(&u, 0.00025);
        }
        // Low speed: r ~ v delta / (L + K v^2) with K v^2 << L
        let r_kin = pl.s.vx * delta.tan() / p.wheelbase_m as f64;
        assert!((pl.s.r - r_kin).abs() / r_kin < 0.06, "r {} kin {}", pl.s.r, r_kin);
    }
}
