//! HELIOS Vehicle Dynamics Control (VDC): per-wheel torque vectoring.
//!
//! Pipeline per 1 ms step (VCU safety core):
//! 1. Quasi-static vertical load estimate (accelerations + aero map).
//! 2. Per-wheel slip ratio from wheel speed and ground velocity at the hub.
//! 3. Longitudinal force demand from pedal map, power/torque/grip envelopes,
//!    brake pedal, coast regen and the VSM decel request.
//! 4. Yaw-rate reference r_ref = v delta / (L + K_us v^2), saturated by the
//!    friction-limited lateral acceleration; PI + feed-forward yaw moment, plus
//!    a sideslip stabilisation term once |beta| exceeds the mode limit.
//! 5. Per-wheel force boxes: motor envelope, regen limit, friction brake,
//!    combined-slip grip circle, TC/ABS ceilings.
//! 6. Weighted least-squares allocation of [Fx, Mz] onto the wheels.
//! 7. Torque split into motor / friction brake; battery power clamp.
#![no_std]
#![forbid(unsafe_code)]

pub mod aero;
pub mod alloc;

use helios_core::consts::G;
use helios_core::math::{Lpf1, Pi, abs, atan2, clamp, cos, powf, sat_sym, sin, sqrt};
use helios_core::{AeroMode, Corner, DriveMode, PerCorner, PowertrainLayout, VehicleParams};

/// Minimum speed used as the slip-ratio denominator, m/s. Below this the
/// controller regulates slip *velocity* (|omega R - v| <= target * V_MIN).
pub const SLIP_V_MIN: f32 = 2.0;
/// Brake pedal travel to deceleration demand gain, m/s^2 at full pedal.
pub const BRAKE_PEDAL_GAIN: f32 = 14.0;
/// Lift-off regen deceleration at regen_fraction = 1, m/s^2.
pub const COAST_REGEN_DECEL: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub struct VdcInputs {
    /// Ground velocity at the CG in the body frame (optical sensor / estimator), m/s.
    pub vx: f32,
    pub vy: f32,
    pub yaw_rate: f32,
    pub ax: f32,
    pub ay: f32,
    /// Mean front road-wheel angle, rad (positive = left).
    pub steer_road: f32,
    pub wheel_omega: PerCorner<f32>,
    pub accel_pedal: f32,
    pub brake_pedal: f32,
    pub mode: DriveMode,
    pub reverse: bool,
    pub torque_enable: bool,
    pub regen_enable: bool,
    pub motor_enable: PerCorner<bool>,
    pub power_scale: f32,
    /// Thermal / inverter derated motor torque limits, Nm (positive magnitudes).
    pub motor_drive_limit: PerCorner<f32>,
    pub motor_regen_limit: PerCorner<f32>,
    /// Battery power limits from the BMS (positive magnitudes), W.
    pub p_discharge_limit: f32,
    pub p_charge_limit: f32,
    /// VSM deceleration request (SafeStop), m/s^2.
    pub decel_request: f32,
    /// Torque vectoring (yaw tracking). Sideslip stabilisation stays active regardless.
    pub tv_enable: bool,
}

impl Default for VdcInputs {
    fn default() -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            yaw_rate: 0.0,
            ax: 0.0,
            ay: 0.0,
            steer_road: 0.0,
            wheel_omega: [0.0; 4],
            accel_pedal: 0.0,
            brake_pedal: 0.0,
            mode: DriveMode::Road,
            reverse: false,
            torque_enable: false,
            regen_enable: false,
            motor_enable: [true; 4],
            power_scale: 1.0,
            motor_drive_limit: [f32::INFINITY; 4],
            motor_regen_limit: [f32::INFINITY; 4],
            p_discharge_limit: f32::INFINITY,
            p_charge_limit: f32::INFINITY,
            decel_request: 0.0,
            tv_enable: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VdcOutputs {
    /// Motor torque command, Nm (+ = forward-propulsive).
    pub motor_torque: PerCorner<f32>,
    /// Friction brake torque command, Nm (>= 0).
    pub brake_torque: PerCorner<f32>,
    pub fx_demand: f32,
    pub mz_demand: f32,
    pub fx_achieved: f32,
    pub mz_achieved: f32,
    pub yaw_rate_ref: f32,
    pub sideslip: f32,
    pub slip: PerCorner<f32>,
    pub tc_active: PerCorner<bool>,
    pub abs_active: PerCorner<bool>,
    pub aero_mode: AeroMode,
    pub fz_est: PerCorner<f32>,
    /// Estimated battery-side electrical power, W (+ = discharge).
    pub p_elec_est: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Vdc {
    pub p: VehicleParams,
    dt: f32,
    ax_f: Lpf1,
    ay_f: Lpf1,
    r_ref_f: Lpf1,
    dr_ref_f: Lpf1,
    r_ref_prev: f32,
    yaw_pi: Pi,
    mu_est: [f32; 4],
    fx_prev: [f32; 4],
    base_brake: [f32; 4],
    aero: aero::AeroStrategy,
}

impl Vdc {
    pub const YAW_BANDWIDTH: f32 = 12.0; // rad/s
    pub const YAW_KI_OVER_KP: f32 = 6.0; // 1/s
    pub const BETA_GAIN: f32 = 3.0e5; // Nm/rad of excess sideslip

    pub fn new(p: VehicleParams, dt: f32) -> Self {
        let kp = p.yaw_inertia_kgm2 * Self::YAW_BANDWIDTH;
        let mz_max = Self::mz_max(&p);
        Self {
            p,
            dt,
            ax_f: Lpf1::new(0.02, dt, 0.0),
            ay_f: Lpf1::new(0.02, dt, 0.0),
            r_ref_f: Lpf1::new(0.05, dt, 0.0),
            dr_ref_f: Lpf1::new(0.02, dt, 0.0),
            r_ref_prev: 0.0,
            yaw_pi: Pi::new(kp, kp * Self::YAW_KI_OVER_KP, -mz_max, mz_max),
            mu_est: [1.5; 4],
            fx_prev: [0.0; 4],
            base_brake: [0.0; 4],
            aero: aero::AeroStrategy::default(),
        }
    }

    /// Bound on the commanded yaw moment: half-track times a 2 g longitudinal
    /// force swing, a generous envelope that the per-wheel boxes trim further.
    fn mz_max(p: &VehicleParams) -> f32 {
        0.5 * p.track_rear_m * 2.0 * p.mass_kg * G
    }

    /// Steady-state yaw moment that makes the linear single-track model hold yaw rate
    /// `r` at speed `v` and road-wheel angle `delta`. With axle cornering stiffnesses
    /// Cf, Cr (N/rad), slip angles af = beta + a r/v - delta, ar = beta - b r/v and
    /// Fy = -C alpha, the lateral and yaw equilibria give
    ///   beta = (Cf delta + r (Cr b - Cf a)/v - m v r) / (Cf + Cr)
    ///   Mz   = a Cf (beta + a r/v - delta) - b Cr (beta - b r/v).
    /// Cornering stiffness per tyre is the MF slope B*C*D at the quasi-static load.
    pub fn mz_steady_state(&self, v: f32, delta: f32, r: f32, fz: &[f32; 4]) -> f32 {
        let p = &self.p;
        let a = p.a();
        let b = p.wheelbase_m - a;
        let t = &p.tyre_mf;
        let c = |k: usize| t.by * t.cy * t.mu(fz[k]) * fz[k];
        let cf = c(0) + c(1);
        let cr = c(2) + c(3);
        let vv = v.max(5.0);
        let beta = (cf * delta + r * (cr * b - cf * a) / vv - p.mass_kg * vv * r) / (cf + cr).max(1.0);
        a * cf * (beta + a * r / vv - delta) - b * cr * (beta - b * r / vv)
    }

    /// Steady-state yaw-rate reference (single-track model) with friction saturation.
    pub fn yaw_rate_reference(&self, v: f32, delta: f32, k_us: f32, mu: f32, aero: AeroMode) -> f32 {
        let l = self.p.wheelbase_m;
        let r_ss = v * delta / (l + k_us * v * v);
        let v_s = v.max(1.0);
        let r_max = 0.85 * self.p.ay_limit(mu, v, aero) / v_s;
        sat_sym(r_ss, r_max)
    }

    pub fn step(&mut self, i: &VdcInputs) -> VdcOutputs {
        let p = self.p;
        let dt = self.dt;
        let prof = i.mode.profile();
        let m = p.mass_kg;
        let v = abs(i.vx);
        let radii = p.radii();
        let ax = self.ax_f.step(i.ax);
        let ay = self.ay_f.step(i.ay);
        let aero_now = self.aero.mode();
        let fz = p.quasi_static_fz(ax, ay, i.vx, aero_now);
        let fz_sum: f32 = fz.iter().sum::<f32>().max(1.0);

        // Wheel-plane kinematics and slip.
        let deltas = p.ackermann(i.steer_road);
        let mut slip = [0.0f32; 4];
        for c in Corner::ALL {
            let k = c.idx();
            let (x, y) = p.corner_xy(c);
            let vxh = i.vx - i.yaw_rate * y;
            let vyh = i.vy + i.yaw_rate * x;
            let v_long = vxh * cos(deltas[k]) + vyh * sin(deltas[k]);
            slip[k] = (i.wheel_omega[k] * radii[k] - v_long) / abs(v_long).max(SLIP_V_MIN);
        }
        let beta = atan2(i.vy, v.max(1.0));

        // ---- Longitudinal demand ----
        let eta = p.inverter_eta * p.motor.eta_corner;
        let mut f_motor_max = 0.0;
        let mut f_grip = 0.0;
        for c in Corner::ALL {
            let k = c.idx();
            if p.layout.driven(c) && i.motor_enable[k] {
                let t = p.motor.torque_limit(i.wheel_omega[k]).min(i.motor_drive_limit[k]);
                f_motor_max += t / radii[k];
                f_grip += prof.mu_assumed * fz[k];
            }
        }
        let p_avail =
            (p.system_peak_power_w * prof.power_fraction * clamp(i.power_scale, 0.0, 1.0)).min(i.p_discharge_limit * eta);
        let f_power = p_avail / v.max(1.0);
        let mut f_drive_max = f_motor_max.min(f_power).min(f_grip);
        let v_gov = if i.reverse { 15.0 / 3.6 } else { prof.vmax };
        if i.reverse {
            f_drive_max = f_drive_max.min(0.2 * m * G);
        }
        let gov = clamp((v_gov - v) / 5.0, 0.0, 1.0);
        let pedal = clamp(i.accel_pedal, 0.0, 1.0);
        let braking = i.brake_pedal > 0.02;
        let mut fx_drive = if i.torque_enable && !braking { powf(pedal, prof.pedal_gamma) * f_drive_max * gov } else { 0.0 };
        if i.reverse {
            fx_drive = -fx_drive;
        }
        let coast = if i.regen_enable && !i.reverse && pedal < 0.02 && !braking {
            -m * COAST_REGEN_DECEL * prof.regen_fraction * clamp((v - 3.0) / 5.0, 0.0, 1.0)
        } else {
            0.0
        };
        let a_brake = (clamp(i.brake_pedal, 0.0, 1.0) * BRAKE_PEDAL_GAIN).max(i.decel_request);
        let brake_dir = if i.vx < -0.3 { 1.0 } else { -1.0 };
        let fx_brake = if v > 0.1 || a_brake > 0.0 { brake_dir * m * a_brake } else { 0.0 };
        let fx_dem = fx_drive + coast + fx_brake;

        // ---- Yaw moment demand ----
        let r_ref_raw = if i.reverse {
            0.0
        } else {
            self.yaw_rate_reference(v, i.steer_road, prof.understeer_gradient, prof.mu_assumed, aero_now)
        };
        let r_ref = self.r_ref_f.step(r_ref_raw);
        let dr_ref = self.dr_ref_f.step((r_ref - self.r_ref_prev) / dt);
        self.r_ref_prev = r_ref;
        let e_r = r_ref - i.yaw_rate;
        let fade = clamp((v - 3.0) / 7.0, 0.0, 1.0);
        let mz_lim = Self::mz_max(&p);
        let mz_track = if i.tv_enable && fade > 0.0 && !i.reverse {
            // The single-track feed-forward is only valid in the linear tyre range: fade it
            // out between 50 % and 85 % of the friction-limited lateral acceleration (the
            // skidpad scenario showed it saturating Mz against the turn at large steer).
            let fz_lin = p.quasi_static_fz(0.0, 0.0, i.vx, aero_now);
            let ay_lim = p.ay_limit(prof.mu_assumed, v, aero_now).max(1.0);
            let lin = clamp((0.85 - abs(r_ref * v) / ay_lim) / 0.35, 0.0, 1.0);
            let mz_ff = lin * sat_sym(self.mz_steady_state(v, i.steer_road, r_ref, &fz_lin), 0.25 * mz_lim);
            fade * (p.yaw_inertia_kgm2 * dr_ref + mz_ff + self.yaw_pi.step(e_r, dt))
        } else {
            self.yaw_pi.reset();
            0.0
        };
        let beta_excess = beta - sat_sym(beta, prof.max_sideslip);
        let mz_beta = if v > 5.0 { Self::BETA_GAIN * beta_excess } else { 0.0 };
        let mz_dem = sat_sym(mz_track + mz_beta, mz_lim);
        let critical = abs(beta_excess) > 0.0 || abs(e_r) > 0.15;

        // ---- Predictive Grip Estimation ----
        for c in Corner::ALL {
            let k = c.idx();
            let fy = m * ay * fz[k] / fz_sum;
            let fx = self.fx_prev[k];
            let f_total = sqrt(fx * fx + fy * fy);
            let utilized_mu = f_total / fz[k].max(1.0);
            
            let slip_mag = abs(slip[k]);
            if slip_mag > prof.slip_target * 0.5 {
                // High confidence (slipping): adapt fast
                let alpha = 2.0 * dt;
                self.mu_est[k] += alpha * (utilized_mu - self.mu_est[k]);
            } else {
                // Low confidence: relax to mode baseline
                let alpha = 0.5 * dt;
                self.mu_est[k] += alpha * (prof.mu_assumed - self.mu_est[k]);
            }
        }

        // ---- Per-wheel force boxes & SMC limits ----
        let mut lo = [0.0f32; 4];
        let mut hi = [0.0f32; 4];
        let mut lo_m = [0.0f32; 4];
        let mut hi_m = [0.0f32; 4];
        let mut caps = [0.0f32; 4];
        let mut tc_active = [false; 4];
        let mut abs_active = [false; 4];
        let mut slip_penalty = [1.0f32; 4];
        
        for c in Corner::ALL {
            let k = c.idx();
            let r = radii[k];
            let omega = i.wheel_omega[k];
            let driven = p.layout.driven(c) && i.motor_enable[k];
            let t_env = p.motor.torque_limit(omega);
            let t_drive = t_env.min(i.motor_drive_limit[k]);
            let t_regen = (p.motor.regen_torque_limit(omega) * prof.regen_fraction).min(i.motor_regen_limit[k]);
            if driven {
                if i.reverse {
                    let t_rev = t_drive.min(0.3 * p.motor.peak_torque_nm);
                    lo_m[k] = if i.torque_enable { -t_rev / r } else { 0.0 };
                    hi_m[k] = if i.regen_enable || i.torque_enable { t_regen.min(t_rev) / r } else { 0.0 };
                } else {
                    hi_m[k] = if i.torque_enable { t_drive / r } else { 0.0 };
                    lo_m[k] = if i.regen_enable { -t_regen / r } else { 0.0 };
                }
            }
            let t_brake = if c.is_front() { p.brake_torque_max_front } else { p.brake_torque_max_rear };
            let (lo_b, hi_b) = if omega >= -0.5 { (-t_brake / r, 0.0) } else { (0.0, t_brake / r) };
            
            // Combined-slip friction circle based on estimated mu
            let cap = self.mu_est[k] * fz[k];
            let fy = m * ay * fz[k] / fz_sum;
            let long_cap = sqrt((cap * cap - fy * fy).max((0.3 * cap) * (0.3 * cap)));
            caps[k] = long_cap.max(1.0);
            
            // SMC Slip Control
            let phi = 0.02; // boundary layer
            let k_smc = 0.8; // reaching law gain fraction
            
            let tc_err = slip[k] - prof.slip_target;
            let tc_sat = clamp(tc_err / phi, 0.0, 1.0);
            let tc_limit = long_cap * (1.0 - k_smc * tc_sat);
            
            let abs_err = -slip[k] - prof.slip_target;
            let abs_sat = clamp(abs_err / phi, 0.0, 1.0);
            let abs_limit = -long_cap * (1.0 - k_smc * abs_sat);
            
            tc_active[k] = tc_err > 0.0;
            abs_active[k] = abs_err > 0.0;
            slip_penalty[k] = 1.0 + 10.0 * tc_sat.max(abs_sat);

            hi[k] = (hi_m[k] + hi_b).min(tc_limit);
            lo[k] = (lo_m[k] + lo_b).max(abs_limit);
            if lo[k] > hi[k] {
                let mid = 0.5 * (lo[k] + hi[k]);
                lo[k] = mid;
                hi[k] = mid;
            }
        }

        // ---- Allocation ----
        let f_s: f32 = caps.iter().sum::<f32>().max(1.0);
        let prio = if critical { 10.0 } else { 1.0 };
        let arm = 0.8;
        let mut prob = alloc::Problem {
            n: 4,
            b: [[0.0; 4]; 2],
            v: [fx_dem / f_s, mz_dem / (arm * f_s)],
            q: [1.0, prio],
            w: [0.0; 4],
            lo: [0.0; 4],
            hi: [0.0; 4],
        };
        const LAMBDA: f32 = 1.0e-3;
        let mut u = [0.0f32; 4];
        match p.layout {
            PowertrainLayout::Quad | PowertrainLayout::DualRear => {
                for c in Corner::ALL {
                    let k = c.idx();
                    let (x, y) = p.corner_xy(c);
                    prob.b[0][k] = cos(deltas[k]);
                    prob.b[1][k] = (x * sin(deltas[k]) - y * cos(deltas[k])) / arm;
                    let bias = if c.is_front() {
                        prof.front_bias
                    } else {
                        1.0 - prof.front_bias
                    };
                    let weight_bias = 1.0 / bias.max(0.01);
                    prob.w[k] = LAMBDA * weight_bias * slip_penalty[k] * (f_s / caps[k]) * (f_s / caps[k]) / 16.0;
                    prob.lo[k] = lo[k] / f_s;
                    prob.hi[k] = hi[k] / f_s;
                }
                let s = alloc::solve(&prob);
                for k in 0..4 {
                    u[k] = s.u[k] * f_s;
                }
            }
            PowertrainLayout::DualAxle => {
                // Open differentials: equal force left/right per axle; no drive-torque yaw authority.
                prob.n = 2;
                prob.q[1] = 0.0;
                for (axle, (a, b)) in [(0usize, (0usize, 1usize)), (1, (2, 3))] {
                    prob.b[0][axle] = cos(deltas[a]) + cos(deltas[b]);
                    let cap = caps[a].min(caps[b]);
                    let bias = if axle == 0 {
                        prof.front_bias
                    } else {
                        1.0 - prof.front_bias
                    };
                    let weight_bias = 1.0 / bias.max(0.01);
                    let max_penalty = slip_penalty[a].max(slip_penalty[b]);
                    prob.w[axle] = LAMBDA * weight_bias * max_penalty * (f_s / cap) * (f_s / cap) / 16.0;
                    prob.lo[axle] = lo[a].max(lo[b]) / f_s;
                    prob.hi[axle] = hi[a].min(hi[b]) / f_s;
                    if prob.lo[axle] > prob.hi[axle] {
                        prob.lo[axle] = prob.hi[axle];
                    }
                }
                let s = alloc::solve(&prob);
                u = [s.u[0] * f_s, s.u[0] * f_s, s.u[1] * f_s, s.u[1] * f_s];
            }
        }

        // ---- Motor / friction split (Regen-ABS) ----
        let mut t_motor = [0.0f32; 4];
        let mut t_brake = [0.0f32; 4];
        for k in 0..4 {
            let r = radii[k];
            let t = u[k] * r;
            
            // Ideal steady-state friction brake demand
            let mut raw_friction = 0.0;
            if t < 0.0 {
                let ideal_motor = t.max(lo_m[k] * r);
                raw_friction = ideal_motor - t;
            }
            
            if raw_friction > self.base_brake[k] || !abs_active[k] {
                // Apply instantly to meet driver demand safely, or release instantly if not in ABS
                self.base_brake[k] = raw_friction;
            } else {
                // ABS is active: hold/release friction brake slowly to let electric motor modulate
                let alpha = match i.mode {
                    DriveMode::Track | DriveMode::Sport => 15.0 * dt, // Fast mechanical feel for racing
                    _ => 2.0 * dt, // Smooth electric ABS on residential/road modes
                };
                self.base_brake[k] += alpha * (raw_friction - self.base_brake[k]);
            }
            
            // Safety: Ensure motor has enough authority to counteract the smoothed friction brake
            let max_safe_brake = (hi_m[k] * r - t).max(0.0);
            if self.base_brake[k] > max_safe_brake {
                self.base_brake[k] = max_safe_brake;
            }
            
            t_brake[k] = self.base_brake[k];
            t_motor[k] = clamp(t + t_brake[k], lo_m[k] * r, hi_m[k] * r);
        }

        // ---- Battery power clamp (two fixed-point passes; losses are quadratic in T) ----
        let p_of = |tm: &[f32; 4]| -> f32 {
            let mut s = 0.0;
            for k in 0..4 {
                let w = i.wheel_omega[k];
                let mech = tm[k] * w;
                let loss = p.motor.loss(tm[k], w);
                let e = mech + loss;
                s += if e >= 0.0 { e / p.inverter_eta } else { e * p.inverter_eta };
            }
            s
        };
        for _ in 0..2 {
            let pe = p_of(&t_motor);
            if pe > i.p_discharge_limit {
                let k_s = clamp(i.p_discharge_limit / pe, 0.0, 1.0);
                for t in t_motor.iter_mut() {
                    if *t * 1.0 > 0.0 {
                        *t *= k_s;
                    }
                }
            } else if pe < -i.p_charge_limit {
                let k_s = clamp(i.p_charge_limit / -pe, 0.0, 1.0);
                for k in 0..4 {
                    if t_motor[k] * i.wheel_omega[k] < 0.0 {
                        let removed = t_motor[k] * (1.0 - k_s);
                        t_motor[k] -= removed;
                        t_brake[k] += abs(removed);
                    }
                }
            }
        }
        let p_elec = p_of(&t_motor);

        let mut fx_ach = 0.0;
        let mut mz_ach = 0.0;
        for c in Corner::ALL {
            let k = c.idx();
            let (x, y) = p.corner_xy(c);
            let brake_sign = if i.wheel_omega[k] >= -0.5 { -1.0 } else { 1.0 };
            let f = (t_motor[k] + brake_sign * t_brake[k]) / radii[k];
            self.fx_prev[k] = f;
            fx_ach += f * cos(deltas[k]);
            mz_ach += f * (x * sin(deltas[k]) - y * cos(deltas[k]));
        }

        let aero_mode = self.aero.step(
            &aero::AeroInputs {
                mode: i.mode,
                speed: v,
                fx_demand_g: fx_dem / (m * G),
                ay,
                steer_road: i.steer_road,
                yaw_error: e_r,
                brake_pedal: i.brake_pedal,
            },
            dt,
        );

        VdcOutputs {
            motor_torque: t_motor,
            brake_torque: t_brake,
            fx_demand: fx_dem,
            mz_demand: mz_dem,
            fx_achieved: fx_ach,
            mz_achieved: mz_ach,
            yaw_rate_ref: r_ref,
            sideslip: beta,
            slip,
            tc_active,
            abs_active,
            aero_mode,
            fz_est: fz,
            p_elec_est: p_elec,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helios_core::consts::DEG;
    use helios_core::params::HELIOS_BASELINE;

    fn rolling(v: f32) -> VdcInputs {
        let p = HELIOS_BASELINE;
        let r = p.radii();
        VdcInputs {
            vx: v,
            wheel_omega: [v / r[0], v / r[1], v / r[2], v / r[3]],
            torque_enable: true,
            regen_enable: true,
            mode: DriveMode::Track,
            ..Default::default()
        }
    }

    fn settle(vdc: &mut Vdc, i: &VdcInputs, n: usize) -> VdcOutputs {
        let mut o = vdc.step(i);
        for _ in 0..n {
            o = vdc.step(i);
        }
        o
    }

    #[test]
    fn straight_full_throttle_symmetric_and_power_limited() {
        let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let mut i = rolling(60.0);
        i.accel_pedal = 1.0;
        let o = settle(&mut vdc, &i, 200);
        assert!(abs(o.motor_torque[0] - o.motor_torque[1]) < 1.0, "{:?}", o.motor_torque);
        assert!(abs(o.motor_torque[2] - o.motor_torque[3]) < 1.0);
        assert!(abs(o.mz_achieved) < 50.0);
        // 1.5 MW system cap at 60 m/s => 25 kN at the contact patches.
        assert!(o.fx_achieved > 23_000.0 && o.fx_achieved < 25_100.0, "{}", o.fx_achieved);
        assert!(o.p_elec_est < 1.75e6);
    }

    #[test]
    fn steady_state_feedforward_is_zero_at_natural_yaw_rate() {
        // The linear single-track model's own steady yaw rate needs no external moment:
        // r_nat = v delta / (L + K v^2) with K = m (b Cr - a Cf) / (L Cf Cr).
        let p = HELIOS_BASELINE;
        let vdc = Vdc::new(p, 0.001);
        let fz = p.quasi_static_fz(0.0, 0.0, 30.0, AeroMode::HighDownforce);
        let t = &p.tyre_mf;
        let c = |k: usize| t.by * t.cy * t.mu(fz[k]) * fz[k];
        let (cf, cr) = (c(0) + c(1), c(2) + c(3));
        let a = p.a();
        let b = p.wheelbase_m - a;
        let l = p.wheelbase_m;
        let k_nat = p.mass_kg * (b * cr - a * cf) / (l * cf * cr);
        let (v, d) = (30.0, 0.02);
        let r_nat = v * d / (l + k_nat * v * v);
        assert!(abs(vdc.mz_steady_state(v, d, r_nat, &fz)) < 1.0, "{}", vdc.mz_steady_state(v, d, r_nat, &fz));
        // Asking for less yaw than natural needs a negative (stabilising) moment.
        assert!(vdc.mz_steady_state(v, d, 0.8 * r_nat, &fz) < 0.0);
    }

    #[test]
    fn yaw_reference_matches_single_track_formula() {
        let vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let v = 25.0;
        let d = 1.0 * DEG;
        let k = 0.0012;
        let r = vdc.yaw_rate_reference(v, d, k, 1.5, AeroMode::HighDownforce);
        let expected = v * d / (2.70 + k * v * v);
        assert!(abs(r - expected) < 1e-5, "{r} vs {expected}");
    }

    #[test]
    fn understeer_produces_positive_yaw_moment_via_right_wheels() {
        let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let mut i = rolling(30.0);
        i.accel_pedal = 0.3;
        i.steer_road = 2.0 * DEG;
        i.yaw_rate = 0.05; // well below the reference => understeer
        i.ay = 1.5;
        let o = settle(&mut vdc, &i, 300);
        assert!(o.yaw_rate_ref > 0.15);
        assert!(o.mz_demand > 0.0 && o.mz_achieved > 0.0, "{} {}", o.mz_demand, o.mz_achieved);
        assert!(o.motor_torque[3] > o.motor_torque[2], "RR > RL: {:?}", o.motor_torque);
    }

    #[test]
    fn oversteer_sideslip_term_counters_rotation() {
        let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let mut i = rolling(30.0);
        i.mode = DriveMode::Road;
        i.steer_road = 1.0 * DEG;
        i.yaw_rate = 0.6;
        i.vy = -30.0 * libm::tanf(10.0 * DEG); // beta = -10 deg, rear stepping out in a left turn
        let o = settle(&mut vdc, &i, 50);
        assert!(o.mz_demand < 0.0, "{}", o.mz_demand);
        assert!(o.mz_achieved < 0.0, "{}", o.mz_achieved);
    }

    #[test]
    fn traction_control_reduces_force_on_spinning_wheel() {
        let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let mut i = rolling(20.0);
        i.accel_pedal = 1.0;
        let base = settle(&mut vdc, &i, 50).motor_torque[2];
        i.wheel_omega[2] *= 1.30; // RL at 30 % slip
        let o = settle(&mut vdc, &i, 100);
        assert!(o.tc_active[2]);
        assert!(o.motor_torque[2] < 0.3 * base, "{} vs {}", o.motor_torque[2], base);
    }

    #[test]
    fn no_propulsion_without_torque_enable() {
        let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let mut i = rolling(15.0);
        i.torque_enable = false;
        i.accel_pedal = 1.0;
        let o = settle(&mut vdc, &i, 20);
        for t in o.motor_torque {
            assert!(t <= 1e-3, "{:?}", o.motor_torque);
        }
    }

    #[test]
    fn braking_blends_regen_first_then_friction() {
        let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let mut i = rolling(40.0);
        i.brake_pedal = 0.2; // 2.8 m/s^2 => 4.2 kN, within regen
        let o = settle(&mut vdc, &i, 50);
        assert!(o.brake_torque.iter().all(|b| *b < 1.0), "pure regen expected: {:?}", o.brake_torque);
        i.brake_pedal = 1.0; // 14 m/s^2 => beyond regen at this speed? regen 0.8 * 4300 Nm per corner
        i.p_charge_limit = 300_000.0;
        let o = settle(&mut vdc, &i, 50);
        assert!(o.brake_torque.iter().any(|b| *b > 100.0), "{:?}", o.brake_torque);
        assert!(o.p_elec_est > -310_000.0, "{}", o.p_elec_est);
    }

    #[test]
    fn battery_discharge_limit_respected() {
        let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let mut i = rolling(50.0);
        i.accel_pedal = 1.0;
        i.p_discharge_limit = 200_000.0;
        let o = settle(&mut vdc, &i, 100);
        assert!(o.p_elec_est <= 201_000.0, "{}", o.p_elec_est);
    }

    #[test]
    fn drift_mode_biases_torque_rearward_and_allows_slip() {
        let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
        let mut i = rolling(20.0);
        i.mode = DriveMode::Drift;
        i.accel_pedal = 0.4; // Demand less than rear grip limit so it doesn't spill over to front
        let o = settle(&mut vdc, &i, 50);
        // Drift mode should be heavily rear biased
        assert!(o.motor_torque[2] > o.motor_torque[0] * 5.0, "Rear motors should drive much more than front");
        assert!(o.motor_torque[2] > 1000.0 && o.motor_torque[3] > 1000.0, "Rear motors should drive heavily");
        
        // Let's induce 20% slip, which is under the 25% target for drift mode
        i.wheel_omega[2] *= 1.20;
        let o2 = settle(&mut vdc, &i, 50);
        assert!(!o2.tc_active[2], "TC should not interfere at 20% slip in drift mode");
    }

    #[test]
    fn dual_rear_layout_never_drives_front() {
        let p = HELIOS_BASELINE.with_layout(PowertrainLayout::DualRear);
        let mut vdc = Vdc::new(p, 0.001);
        let mut i = rolling(30.0);
        i.accel_pedal = 1.0;
        let o = settle(&mut vdc, &i, 200);
        assert!(abs(o.motor_torque[0]) < 1e-3 && abs(o.motor_torque[1]) < 1e-3);
        assert!(o.motor_torque[2] > 0.0 && o.motor_torque[3] > 0.0);
    }
}
