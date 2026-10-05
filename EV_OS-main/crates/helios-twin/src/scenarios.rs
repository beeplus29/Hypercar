//! Test scenarios with quantitative pass criteria.

use crate::sim::Sim;
use helios_core::params::{HELIOS_BASELINE, VehicleParams};
use helios_core::{DriveMode, Gear, PowertrainLayout};
use helios_vsm::VehicleState;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

#[allow(missing_debug_implementations)]
#[derive(Serialize)]
pub struct ScenarioResult {
    pub name: String,
    pub layout: String,
    pub metrics: BTreeMap<String, f64>,
    pub checks: Vec<Check>,
    pub sim_seconds: f64,
    pub wall_seconds: f64,
    #[serde(skip)]
    pub sim: Option<Box<Sim>>,
}

impl ScenarioResult {
    pub fn pass(&self) -> bool {
        self.checks.iter().all(|c| c.pass)
    }
}

struct Ctx {
    name: String,
    layout: String,
    m: BTreeMap<String, f64>,
    checks: Vec<Check>,
    t0: std::time::Instant,
}

impl Ctx {
    fn new(name: &str, layout: PowertrainLayout) -> Self {
        Self {
            name: name.into(),
            layout: format!("{layout:?}"),
            m: BTreeMap::new(),
            checks: vec![],
            t0: std::time::Instant::now(),
        }
    }
    fn metric(&mut self, k: &str, v: f64) {
        self.m.insert(k.into(), v);
    }
    fn check(&mut self, name: &str, pass: bool, detail: String) {
        self.checks.push(Check { name: name.into(), pass, detail });
    }
    fn done(self, sim: Sim) -> ScenarioResult {
        ScenarioResult {
            name: self.name,
            layout: self.layout,
            metrics: self.m,
            checks: self.checks,
            sim_seconds: sim.t,
            wall_seconds: self.t0.elapsed().as_secs_f64(),
            sim: Some(Box::new(sim)),
        }
    }
}

pub fn params(layout: PowertrainLayout) -> VehicleParams {
    HELIOS_BASELINE.with_layout(layout)
}

/// Driver speed-hold: P on speed error into the accelerator / brake.
fn hold_speed(sim: &mut Sim, v_target: f64) {
    let e = v_target - sim.speed();
    if e >= 0.0 {
        sim.driver.accel_pedal = (0.08 + 0.15 * e).clamp(0.0, 1.0);
        sim.driver.brake_pedal = 0.0;
    } else {
        sim.driver.accel_pedal = (0.08 + 0.15 * e).clamp(0.0, 1.0);
        sim.driver.brake_pedal = if e < -1.0 { (-0.05 * e).clamp(0.0, 0.3) } else { 0.0 };
    }
}

pub fn launch(layout: PowertrainLayout) -> anyhow::Result<ScenarioResult> {
    let mut c = Ctx::new("launch_0_300", layout);
    let mut sim = Sim::new(params(layout), 0.9, 25.0);
    let t_ready = sim.power_up(DriveMode::Track)?;
    c.metric("t_ready_s", t_ready);
    sim.driver.accel_pedal = 1.0;
    let t0 = sim.t;
    let mut marks: [Option<(f64, f64)>; 3] = [None; 3];
    let mut p_peak: f64 = 0.0;
    let mut slip_peak: f64 = 0.0;
    let mut ax_peak: f64 = 0.0;
    let mut dist = 0.0;
    while sim.t - t0 < 40.0 {
        sim.step();
        dist += sim.speed() * 1e-3;
        p_peak = p_peak.max(sim.p_batt);
        ax_peak = ax_peak.max(sim.plant.aux.ax);
        for k in 0..4 {
            if sim.speed() > 3.0 {
                slip_peak = slip_peak.max(sim.plant.aux.slip_ss[k]);
            }
        }
        let kph = sim.speed() * 3.6;
        for (i, lim) in [100.0, 200.0, 300.0].iter().enumerate() {
            if marks[i].is_none() && kph >= *lim {
                marks[i] = Some((sim.t - t0, dist));
            }
        }
        if marks[2].is_some() {
            break;
        }
    }
    let names = ["0_100", "0_200", "0_300"];
    for (i, n) in names.iter().enumerate() {
        if let Some((t, d)) = marks[i] {
            c.metric(&format!("t_{n}_kph_s"), t);
            c.metric(&format!("d_{n}_kph_m"), d);
        }
    }
    c.metric("p_batt_peak_kw", p_peak / 1000.0);
    c.metric("ax_peak_g", ax_peak / 9.80665);
    c.metric("slip_peak", slip_peak);
    c.metric("t_cell_end_c", sim.battery.s.t_cell_c as f64);
    c.metric("soc_used_pct", (0.9 - sim.battery.s.soc as f64) * 100.0);
    let t100 = marks[0].map(|m| m.0).unwrap_or(f64::INFINITY);
    let target = if layout == PowertrainLayout::Quad { 2.2 } else { 3.2 };
    c.check("0-100 km/h", t100 <= target, format!("{t100:.2} s (target <= {target} s)"));
    c.check("TC slip bounded", slip_peak < 0.25, format!("peak steady-state slip {slip_peak:.3}"));
    c.check("battery power <= SOP", p_peak <= 1.65e6, format!("{:.0} kW", p_peak / 1e3));
    Ok(c.done(sim))
}

pub fn braking(layout: PowertrainLayout) -> anyhow::Result<ScenarioResult> {
    let mut c = Ctx::new("brake_200_0", layout);
    let mut sim = Sim::new(params(layout), 0.6, 25.0);
    sim.power_up(DriveMode::Track)?;
    let v0 = 200.0 / 3.6;
    sim.set_speed(v0);
    for _ in 0..500 {
        hold_speed(&mut sim, v0);
        sim.step();
    }
    let e_regen0 = sim.e_regen_j;
    sim.driver.accel_pedal = 0.0;
    sim.driver.brake_pedal = 1.0;
    let x0 = sim.plant.s.x;
    let t0 = sim.t;
    let mut a_peak: f64 = 0.0;
    let mut abs_seen = false;
    let mut airbrake = false;
    sim.run_until(15.0, |s| {
        s.speed() < 0.3
    });
    for smp in sim.log.iter().filter(|s| s.t > t0) {
        a_peak = a_peak.max(-smp.ax);
        airbrake |= smp.aero == 2;
    }
    abs_seen |= sim.vdc_out.abs_active.iter().any(|a| *a);
    let d = sim.plant.s.x - x0;
    let t = sim.t - t0;
    c.metric("stop_distance_m", d);
    c.metric("stop_time_s", t);
    c.metric("mean_decel_g", v0 / t / 9.80665);
    c.metric("peak_decel_g", a_peak / 9.80665);
    c.metric("regen_energy_kj", (sim.e_regen_j - e_regen0) / 1000.0);
    c.metric("kinetic_energy_kj", 0.5 * sim.p.mass_kg as f64 * v0 * v0 / 1000.0);
    c.check("200-0 distance", d < 115.0, format!("{d:.1} m (target < 115 m)"));
    c.check("airbrake deployed", airbrake, "aero mode 2 observed".into());
    let _ = abs_seen;
    Ok(c.done(sim))
}

fn step_steer_run(layout: PowertrainLayout, tv: bool) -> anyhow::Result<(Sim, BTreeMap<String, f64>)> {
    let mut sim = Sim::new(params(layout), 0.7, 25.0);
    sim.power_up(DriveMode::Road)?;
    sim.driver.tv_enable = tv;
    let v = 100.0 / 3.6;
    sim.set_speed(v);
    for _ in 0..1500 {
        hold_speed(&mut sim, v);
        sim.step();
    }
    // Steer for ~4 m/s^2 using the Road-mode target gradient.
    let k = DriveMode::Road.profile().understeer_gradient as f64;
    let l = sim.p.wheelbase_m as f64;
    let delta = 4.0 * (l + k * v * v) / (v * v);
    let t0 = sim.t;
    let mut r_trace = vec![];
    while sim.t - t0 < 4.0 {
        let tau = sim.t - t0;
        sim.driver.steer_road = delta * (tau / 0.1).clamp(0.0, 1.0);
        hold_speed(&mut sim, v);
        sim.step();
        r_trace.push((tau, sim.plant.s.r));
    }
    let r_ss = r_trace.iter().filter(|(t, _)| *t > 3.0).map(|(_, r)| *r).sum::<f64>()
        / r_trace.iter().filter(|(t, _)| *t > 3.0).count() as f64;
    let t50 = 0.05;
    let t90 = r_trace.iter().find(|(_, r)| *r >= 0.9 * r_ss).map(|(t, _)| *t).unwrap_or(f64::NAN);
    let r_peak = r_trace.iter().map(|(_, r)| *r).fold(f64::MIN, f64::max);
    let mut m = BTreeMap::new();
    m.insert("delta_road_deg".into(), delta.to_degrees());
    m.insert("yaw_rate_ss".into(), r_ss);
    m.insert("yaw_rate_ref_ss".into(), sim.vdc_out.yaw_rate_ref as f64);
    m.insert("ay_ss".into(), sim.plant.aux.ay);
    m.insert("response_time_s".into(), t90 - t50);
    m.insert("overshoot_pct".into(), (r_peak / r_ss - 1.0) * 100.0);
    Ok((sim, m))
}

pub fn step_steer(layout: PowertrainLayout) -> anyhow::Result<ScenarioResult> {
    let mut c = Ctx::new("step_steer_iso7401", layout);
    let (_off, m_off) = step_steer_run(layout, false)?;
    let (sim, m_on) = step_steer_run(layout, true)?;
    for (k, v) in &m_off {
        c.metric(&format!("tv_off_{k}"), *v);
    }
    for (k, v) in &m_on {
        c.metric(&format!("tv_on_{k}"), *v);
    }
    let tr_on = m_on["response_time_s"];
    let tr_off = m_off["response_time_s"];
    let err_on = (m_on["yaw_rate_ss"] / m_on["yaw_rate_ref_ss"] - 1.0).abs();
    c.check("TV tracks yaw reference", err_on < 0.05, format!("steady error {:.1} %", err_on * 100.0));
    c.check("TV response <= passive", tr_on <= tr_off + 1e-3, format!("{tr_on:.3} s vs {tr_off:.3} s"));
    c.check("overshoot < 15 %", m_on["overshoot_pct"] < 15.0, format!("{:.1} %", m_on["overshoot_pct"]));
    Ok(c.done(sim))
}

pub fn skidpad(layout: PowertrainLayout) -> anyhow::Result<ScenarioResult> {
    let mut c = Ctx::new("skidpad_r60", layout);
    let mut sim = Sim::new(params(layout), 0.7, 25.0);
    sim.power_up(DriveMode::Track)?;
    let radius = 60.0;
    let mut v_tgt: f64 = 18.0;
    sim.set_speed(v_tgt);
    let l = sim.p.wheelbase_m as f64;
    let mut integ = 0.0;
    let mut ay_best: f64 = 0.0;
    let mut v_best: f64 = 0.0;
    let t0 = sim.t;
    let mut lost = 0usize;
    while sim.t - t0 < 60.0 {
        let v = sim.speed().max(1.0);
        let curv_err = 1.0 / radius - sim.plant.s.r / v;
        integ = (integ + 2.0 * curv_err * 1e-3).clamp(-0.2, 0.2);
        let delta = (l / radius + 8.0 * curv_err * l + integ * l * 4.0).clamp(-0.5, 0.5);
        sim.driver.steer_road = delta;
        hold_speed(&mut sim, v_tgt);
        v_tgt += 0.4e-3; // 0.4 m/s^2 ramp: quasi-steady (ax << ay)
        sim.step();
        let on_path = (curv_err * radius).abs() < 0.03;
        let beta = sim.plant.s.vy.atan2(sim.plant.s.vx).abs();
        if sim.t - t0 > 3.0 && on_path && beta < 6f64.to_radians() {
            let ay = sim.speed() * sim.plant.s.r;
            if ay > ay_best {
                ay_best = ay;
                v_best = sim.speed();
            }
            lost = 0;
        } else if sim.t - t0 > 3.0 {
            lost += 1;
            if lost > 1500 {
                break;
            }
        }
    }
    // Analytic steady-state friction capacity at v_best: fixed point of
    // ay = sum_i mu(Fz_i(ay)) Fz_i(ay) / m with lateral load transfer and downforce.
    let p = sim.p;
    let mut ay_cap: f32 = 10.0;
    for _ in 0..50 {
        let fz = p.quasi_static_fz(0.0, ay_cap, v_best as f32, helios_core::AeroMode::HighDownforce);
        ay_cap = fz.iter().map(|f| p.tyre_mf.mu(*f) * f).sum::<f32>() / p.mass_kg;
    }
    let ratio = ay_best / ay_cap as f64;
    c.metric("ay_max_g", ay_best / 9.80665);
    c.metric("ay_capacity_g", ay_cap as f64 / 9.80665);
    c.metric("grip_utilisation", ratio);
    c.metric("v_at_ay_max_kph", v_best * 3.6);
    c.check(
        "lateral grip >= 85 % of tyre capacity",
        ratio >= 0.85,
        format!("{:.2} g of {:.2} g capacity ({:.0} %) at {:.0} km/h", ay_best / 9.80665, ay_cap / 9.80665, ratio * 100.0, v_best * 3.6),
    );
    Ok(c.done(sim))
}

/// Sine-with-dwell (FMVSS 126 / UN R140 manoeuvre shape) at 80 km/h.
pub fn sine_with_dwell(layout: PowertrainLayout) -> anyhow::Result<ScenarioResult> {
    let mut c = Ctx::new("sine_with_dwell", layout);
    let mut sim = Sim::new(params(layout), 0.7, 25.0);
    sim.power_up(DriveMode::Road)?;
    let v = 80.0 / 3.6;
    sim.set_speed(v);
    for _ in 0..1500 {
        hold_speed(&mut sim, v);
        sim.step();
    }
    let k = DriveMode::Road.profile().understeer_gradient as f64;
    let l = sim.p.wheelbase_m as f64;
    let d03 = 0.3 * 9.80665 * (l + k * v * v) / (v * v);
    let amp = 6.5 * d03;
    let f = 0.7;
    let period = 1.0 / f;
    // Sine to the second peak (3/4 period), dwell 0.5 s, then the final quarter period.
    let t_q3 = 0.75 * period;
    let t_cos = t_q3 + 0.5 + 0.25 * period;
    let t0 = sim.t;
    sim.driver.accel_pedal = 0.0;
    let y0 = sim.plant.s.y;
    let mut r_peak: f64 = 0.0;
    let mut lat_at_107 = f64::NAN;
    let mut r_at = [f64::NAN; 2];
    while sim.t - t0 < t_cos + 2.0 {
        let tau = sim.t - t0;
        let d = if tau < t_q3 {
            amp * (2.0 * std::f64::consts::PI * f * tau).sin()
        } else if tau < t_q3 + 0.5 {
            -amp
        } else if tau < t_cos {
            amp * (2.0 * std::f64::consts::PI * f * (tau - 0.5)).sin()
        } else {
            0.0
        };
        sim.driver.steer_road = d;
        sim.step();
        let r = sim.plant.s.r;
        if tau > 0.25 * period && tau < t_cos {
            r_peak = r_peak.max(r.abs());
        }
        if lat_at_107.is_nan() && tau >= 1.07 {
            lat_at_107 = (sim.plant.s.y - y0).abs();
        }
        if r_at[0].is_nan() && tau >= t_cos + 1.0 {
            r_at[0] = r.abs();
        }
        if r_at[1].is_nan() && tau >= t_cos + 1.75 {
            r_at[1] = r.abs();
        }
    }
    let ratio1 = r_at[0] / r_peak * 100.0;
    let ratio175 = r_at[1] / r_peak * 100.0;
    c.metric("steer_amplitude_road_deg", amp.to_degrees());
    c.metric("handwheel_amplitude_deg", amp.to_degrees() * sim.p.steering_ratio as f64);
    c.metric("yaw_peak_rad_s", r_peak);
    c.metric("yaw_ratio_1s_pct", ratio1);
    c.metric("yaw_ratio_1p75s_pct", ratio175);
    c.metric("lateral_disp_1p07s_m", lat_at_107);
    c.check("yaw rate @1.0 s <= 35 %", ratio1 <= 35.0, format!("{ratio1:.1} %"));
    c.check("yaw rate @1.75 s <= 20 %", ratio175 <= 20.0, format!("{ratio175:.1} %"));
    c.check("lateral displacement >= 1.83 m", lat_at_107 >= 1.83, format!("{lat_at_107:.2} m"));
    Ok(c.done(sim))
}

pub fn thermal_cycles(layout: PowertrainLayout) -> anyhow::Result<ScenarioResult> {
    let mut c = Ctx::new("thermal_8_cycles", layout);
    let mut sim = Sim::new(params(layout), 0.9, 30.0);
    sim.log_every = 50;
    sim.power_up(DriveMode::Track)?;
    sim.set_speed(80.0 / 3.6);
    let t_cell0 = sim.battery.s.t_cell_c as f64;
    let soc0 = sim.battery.s.soc as f64;
    let sop0 = sim.sop.p_discharge_w as f64;
    let mut t_wind_max: f64 = 0.0;
    let mut cycle_times = vec![];
    for _ in 0..8 {
        sim.driver.brake_pedal = 0.0;
        sim.driver.accel_pedal = 1.0;
        let t = sim.run_until(20.0, |s| s.speed() >= 250.0 / 3.6).unwrap_or(f64::NAN);
        cycle_times.push(t);
        sim.driver.accel_pedal = 0.0;
        sim.driver.brake_pedal = 0.9;
        sim.run_until(10.0, |s| s.speed() <= 80.0 / 3.6);
        t_wind_max = t_wind_max.max(sim.motor_th.iter().map(|m| m.t_winding_c as f64).fold(0.0, f64::max));
    }
    c.metric("t_cell_start_c", t_cell0);
    c.metric("t_cell_end_c", sim.battery.s.t_cell_c as f64);
    c.metric("t_coolant_end_c", sim.battery.s.t_coolant_c as f64);
    c.metric("t_winding_max_c", t_wind_max);
    c.metric("soc_used_pct", (soc0 - sim.battery.s.soc as f64) * 100.0);
    c.metric("sop_start_kw", sop0 / 1000.0);
    c.metric("sop_end_kw", sim.sop.p_discharge_w as f64 / 1000.0);
    c.metric("t_80_250_first_s", cycle_times[0]);
    c.metric("t_80_250_last_s", *cycle_times.last().unwrap_or(&f64::NAN));
    c.metric("soc_est_error_pct", (sim.ekf.soc() - sim.battery.s.soc) as f64 * 100.0);
    c.metric("energy_out_kwh", sim.e_out_j / 3.6e6);
    c.metric("energy_regen_kwh", sim.e_regen_j / 3.6e6);
    c.check("cell temperature < 60 C", (sim.battery.s.t_cell_c as f64) < 60.0, format!("{:.1} C", sim.battery.s.t_cell_c));
    c.check("winding < 180 C", t_wind_max < 180.0, format!("{t_wind_max:.0} C"));
    c.check(
        "SOC estimator error < 2 %",
        (sim.ekf.soc() - sim.battery.s.soc).abs() < 0.02,
        format!("{:.2} %", (sim.ekf.soc() - sim.battery.s.soc) * 100.0),
    );
    Ok(c.done(sim))
}

pub fn fault_motor_loss(layout: PowertrainLayout) -> anyhow::Result<ScenarioResult> {
    let mut c = Ctx::new("fault_rl_motor_loss_180kph", layout);
    let mut sim = Sim::new(params(layout), 0.8, 25.0);
    sim.power_up(DriveMode::Track)?;
    let v = 180.0 / 3.6;
    sim.set_speed(v);
    for _ in 0..1000 {
        hold_speed(&mut sim, v);
        sim.step();
    }
    sim.driver.accel_pedal = 0.8;
    sim.run_for(0.2);
    let y0 = sim.plant.s.y;
    let psi0 = sim.plant.s.psi;
    sim.inj.motor_dead[2] = true;
    let t0 = sim.t;
    let (mut r_max, mut beta_max): (f64, f64) = (0.0, 0.0);
    while sim.t - t0 < 2.0 {
        sim.step();
        r_max = r_max.max(sim.plant.s.r.abs());
        beta_max = beta_max.max(sim.plant.s.vy.atan2(sim.plant.s.vx).abs());
    }
    let dy = (sim.plant.s.y - y0).abs();
    c.metric("yaw_rate_peak_deg_s", r_max.to_degrees());
    c.metric("sideslip_peak_deg", beta_max.to_degrees());
    c.metric("lateral_drift_2s_m", dy);
    c.metric("heading_change_deg", (sim.plant.s.psi - psi0).to_degrees());
    c.metric("final_state", sim.vsm.state() as u8 as f64);
    c.metric("final_mode", sim.vsm_out.drive_mode as u8 as f64);
    let expected = if layout == PowertrainLayout::Quad { VehicleState::Drive } else { VehicleState::SafeStop };
    c.check("state reaction", sim.vsm.state() == expected || sim.vsm.state() == VehicleState::HvShutdown && expected == VehicleState::SafeStop,
        format!("{:?} (expected {:?})", sim.vsm.state(), expected));
    c.check("lateral drift < 0.5 m in 2 s", dy < 0.5, format!("{dy:.3} m"));
    c.check("yaw rate < 2 deg/s", r_max.to_degrees() < 2.0, format!("{:.2} deg/s", r_max.to_degrees()));
    Ok(c.done(sim))
}

pub fn fault_hvil(layout: PowertrainLayout) -> anyhow::Result<ScenarioResult> {
    let mut c = Ctx::new("fault_hvil_open_150kph", layout);
    let mut sim = Sim::new(params(layout), 0.8, 25.0);
    sim.log_every = 1;
    sim.power_up(DriveMode::Road)?;
    let v = 150.0 / 3.6;
    sim.set_speed(v);
    for _ in 0..500 {
        hold_speed(&mut sim, v);
        sim.step();
    }
    sim.driver.accel_pedal = 0.4;
    sim.inj.hvil_open = true;
    let t0 = sim.t;
    let t_open = sim.run_until(1.0, |s| !s.vsm_out.contactors.main_pos).unwrap_or(f64::NAN);
    let t_60v = sim.run_until(5.0, |s| s.hv.v_link < 60.0).map(|t| t + t_open).unwrap_or(f64::NAN);
    sim.run_until(30.0, |s| s.speed() < 0.3);
    let t_stop = sim.t - t0;
    c.metric("t_contactors_open_ms", t_open * 1000.0);
    c.metric("t_dc_link_below_60v_ms", t_60v * 1000.0);
    c.metric("t_to_standstill_s", t_stop);
    c.metric("final_state", sim.vsm.state() as u8 as f64);
    c.check("contactors open <= 122 ms", t_open * 1000.0 <= 122.0, format!("{:.0} ms", t_open * 1000.0));
    c.check("DC link < 60 V <= 2 s (UN R94 limit 60 s)", t_60v <= 2.0, format!("{:.0} ms", t_60v * 1000.0));
    c.check("vehicle brought to rest", t_stop < 30.0, format!("{t_stop:.1} s"));
    // HVIL is non-latching: HV stays off and restart is inhibited until the loop is restored.
    sim.driver.brake_pedal = 0.5;
    sim.driver.start_button = true;
    sim.run_for(0.5);
    sim.driver.start_button = false;
    let st = sim.vsm.state();
    c.check(
        "restart inhibited while HVIL open",
        !sim.vsm_out.contactors.main_pos && !sim.vsm_out.contactors.precharge && st != VehicleState::Precharge,
        format!("{st:?}, contactors open"),
    );
    let _ = Gear::Park;
    Ok(c.done(sim))
}

pub type ScenarioFn = fn(PowertrainLayout) -> anyhow::Result<ScenarioResult>;

pub const ALL: &[(&str, ScenarioFn)] = &[
    ("launch", launch),
    ("braking", braking),
    ("step_steer", step_steer),
    ("skidpad", skidpad),
    ("sine_with_dwell", sine_with_dwell),
    ("thermal", thermal_cycles),
    ("motor_loss", fault_motor_loss),
    ("hvil", fault_hvil),
];
