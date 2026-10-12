//! Closed-loop simulation: plant + HV circuit + battery/thermal  <->  VSM + VDC + BMS.
//!
//! Timing: plant RK4 at 4 kHz; VSM, VDC, SOC EKF and HV circuit at 1 kHz;
//! state-of-power at 100 Hz. Control code is the exact `no_std` crates that run
//! on the VCU, called through their public APIs.

use crate::plant::{Plant, PlantInputs};
use helios_core::params::VehicleParams;
use helios_core::{AeroMode, Corner, DriveMode, Gear};
use helios_energy::{Battery, CoolantLoop, MotorThermal, PowerLimits, SocEkf, state_of_power};
use helios_vdc::{Vdc, VdcInputs, VdcOutputs};
use helios_vsm::{VehicleState, Vsm, VsmConfig, VsmInputs, VsmOutputs};
use serde::Serialize;

pub const DT_CTRL: f64 = 0.001;
pub const PLANT_SUBSTEPS: usize = 4;
pub const AUX_LOAD_W: f64 = 2500.0;

/// HV DC-link: 2 mF, 50 ohm precharge, 200 ohm active discharge, 50 kohm passive bleed.
#[derive(Clone, Copy, Debug)]
pub struct HvCircuit {
    pub v_link: f64,
    pub c_f: f64,
    pub r_pre: f64,
    pub r_dis: f64,
    pub r_bleed: f64,
    pub welded: bool,
}

impl Default for HvCircuit {
    fn default() -> Self {
        Self { v_link: 0.0, c_f: 2.0e-3, r_pre: 50.0, r_dis: 200.0, r_bleed: 50.0e3, welded: false }
    }
}

impl HvCircuit {
    pub fn step(&mut self, o: &VsmOutputs, v_pack: f64, dt: f64) {
        let k = &o.contactors;
        if self.welded || (k.main_pos && k.main_neg) {
            self.v_link = v_pack;
            return;
        }
        if k.precharge && k.main_neg {
            let tau = self.r_pre * self.c_f;
            self.v_link += (v_pack - self.v_link) * (1.0 - (-dt / tau).exp());
            return;
        }
        let r = if o.active_discharge { 1.0 / (1.0 / self.r_dis + 1.0 / self.r_bleed) } else { self.r_bleed };
        self.v_link *= (-dt / (r * self.c_f)).exp();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DriverCmd {
    pub wake: bool,
    pub start_button: bool,
    pub accel_pedal: f64,
    pub brake_pedal: f64,
    pub gear_request: Option<Gear>,
    pub mode_request: Option<DriveMode>,
    pub steer_road: f64,
    pub tv_enable: bool,
}

impl Default for DriverCmd {
    fn default() -> Self {
        Self {
            wake: true,
            start_button: false,
            accel_pedal: 0.0,
            brake_pedal: 0.0,
            gear_request: None,
            mode_request: None,
            steer_road: 0.0,
            tv_enable: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Injected {
    pub motor_dead: [bool; 4],
    pub hvil_open: bool,
    pub crash: bool,
    pub isolation_ohm: f64,
    pub mu_scale: f64,
}

impl Default for Injected {
    fn default() -> Self {
        Self { motor_dead: [false; 4], hvil_open: false, crash: false, isolation_ohm: 5.0e6, mu_scale: 1.0 }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Sample {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub speed_kph: f64,
    pub vx: f64,
    pub vy: f64,
    pub yaw_rate: f64,
    pub yaw_rate_ref: f64,
    pub beta_deg: f64,
    pub ax: f64,
    pub ay: f64,
    pub delta_deg: f64,
    pub accel: f64,
    pub brake: f64,
    pub state: u8,
    pub mode: u8,
    pub aero: u8,
    pub tm_fl: f64,
    pub tm_fr: f64,
    pub tm_rl: f64,
    pub tm_rr: f64,
    pub tb_fl: f64,
    pub tb_fr: f64,
    pub tb_rl: f64,
    pub tb_rr: f64,
    pub slip_fl: f64,
    pub slip_fr: f64,
    pub slip_rl: f64,
    pub slip_rr: f64,
    pub fz_fl: f64,
    pub fz_fr: f64,
    pub fz_rl: f64,
    pub fz_rr: f64,
    pub mz_dem: f64,
    pub mz_ach: f64,
    pub p_batt_kw: f64,
    pub i_batt: f64,
    pub v_pack: f64,
    pub v_link: f64,
    pub soc: f64,
    pub soc_est: f64,
    pub t_cell: f64,
    pub t_coolant: f64,
    pub t_wind_max: f64,
    pub p_dis_lim_kw: f64,
    pub cd_a: f64,
    pub cl_a: f64,
    pub downforce: f64,
    pub main_pos: bool,
    pub torque_enable: bool,
}

#[allow(missing_debug_implementations)]
pub struct Sim {
    pub p: VehicleParams,
    pub plant: Plant,
    pub battery: Battery,
    pub hv: HvCircuit,
    pub motor_th: [MotorThermal; 4],
    pub vsm: Vsm,
    pub vdc: Vdc,
    pub ekf: SocEkf,
    pub sop: PowerLimits,
    pub driver: DriverCmd,
    pub inj: Injected,
    pub t: f64,
    pub t_amb_c: f64,
    pub vsm_out: VsmOutputs,
    pub vdc_out: VdcOutputs,
    pub i_batt: f64,
    pub p_batt: f64,
    pub e_out_j: f64,
    pub e_regen_j: f64,
    pub log: Vec<Sample>,
    pub log_every: u32,
    tick: u64,
}

impl Sim {
    pub fn new(p: VehicleParams, soc0: f32, t0_c: f32) -> Self {
        let tolerated = if p.layout.motor_count() == 4 { 1 } else { 0 };
        let mut cfg = VsmConfig::helios_baseline();
        cfg.tolerated_motor_losses = tolerated;
        let mut vsm = Vsm::new(cfg);
        let vsm_out = vsm.step(&VsmInputs::default());
        Self {
            p,
            plant: Plant::new(p),
            battery: Battery::new(p.battery, CoolantLoop::HELIOS, soc0, t0_c),
            hv: HvCircuit::default(),
            motor_th: [MotorThermal { t_winding_c: t0_c }; 4],
            vsm,
            vdc: Vdc::new(p, DT_CTRL as f32),
            ekf: SocEkf::new(p.battery, soc0),
            sop: state_of_power(
                &p.battery,
                &Battery::new(p.battery, CoolantLoop::HELIOS, soc0, t0_c).s,
                10.0,
            ),
            driver: DriverCmd::default(),
            inj: Injected::default(),
            t: 0.0,
            t_amb_c: 25.0,
            vsm_out,
            vdc_out: VdcOutputs::default(),
            i_batt: 0.0,
            p_batt: 0.0,
            e_out_j: 0.0,
            e_regen_j: 0.0,
            log: Vec::new(),
            log_every: 10,
            tick: 0,
        }
    }

    pub fn speed(&self) -> f64 {
        self.plant.s.vx
    }

    /// Place the car at speed `v` (m/s) with wheels rolling; used to start dynamic tests.
    pub fn set_speed(&mut self, v: f64) {
        self.plant.s.vx = v;
        for c in Corner::ALL {
            self.plant.s.omega[c.idx()] = v / self.p.rolling_radius(c) as f64;
        }
    }

    pub fn step(&mut self) {
        let pl = &self.plant;
        let s = pl.s;
        let v_pack_ocv = self.battery.terminal_voltage(0.0) as f64;
        let derate: [f32; 4] = core::array::from_fn(|k| self.motor_th[k].derate(&self.p.motor));
        let vin = VsmInputs {
            wake: self.driver.wake,
            start_button: self.driver.start_button,
            brake_pressed: self.driver.brake_pedal > 0.05,
            accel_pedal: self.driver.accel_pedal as f32,
            gear_request: self.driver.gear_request,
            mode_request: self.driver.mode_request,
            speed_mps: s.vx as f32,
            pack_voltage: v_pack_ocv as f32,
            dc_link_voltage: self.hv.v_link as f32,
            pack_current: self.i_batt as f32,
            hvil_closed: !self.inj.hvil_open,
            isolation_ohm: self.inj.isolation_ohm as f32,
            isolation_valid: true,
            crash: self.inj.crash,
            charge_connector: false,
            charge_request: false,
            bms_critical: self.battery.s.t_cell_c > 65.0,
            bms_warning: false,
            motor_fault: self.inj.motor_dead,
            lv_voltage: 13.8,
            heartbeat_age_ms: 0,
            thermal_derate: self.sop.thermal_derate || derate.iter().any(|d| *d < 1.0),
            steering_sensor_fault: false,
            ground_speed_invalid: false,
            service_reset: false,
            advisory_firewall_trip: false,
        };
        let vo = self.vsm.step(&vin);
        let vdc_in = VdcInputs {
            vx: s.vx as f32,
            vy: s.vy as f32,
            yaw_rate: s.r as f32,
            ax: pl.aux.ax as f32,
            ay: pl.aux.ay as f32,
            steer_road: self.driver.steer_road as f32,
            wheel_omega: s.omega.map(|w| w as f32),
            accel_pedal: self.driver.accel_pedal as f32,
            brake_pedal: self.driver.brake_pedal as f32,
            mode: vo.drive_mode,
            reverse: vo.gear == Gear::Reverse,
            torque_enable: vo.torque_enable,
            regen_enable: vo.regen_enable,
            motor_enable: vo.motor_enable,
            power_scale: vo.power_scale,
            motor_drive_limit: core::array::from_fn(|k| self.p.motor.peak_torque_nm * derate[k]),
            motor_regen_limit: core::array::from_fn(|k| self.p.motor.peak_torque_nm * derate[k]),
            p_discharge_limit: self.sop.p_discharge_w - AUX_LOAD_W as f32,
            p_charge_limit: self.sop.p_charge_w,
            decel_request: vo.decel_request,
            tv_enable: self.driver.tv_enable,
        };
        let co = self.vdc.step(&vdc_in);
        let hv_on = vo.contactors.main_pos && vo.contactors.main_neg;
        let motors_live = hv_on && (vo.torque_enable || vo.regen_enable);
        let pin = PlantInputs {
            motor_cmd: core::array::from_fn(|k| {
                if motors_live && vo.motor_enable[k] { co.motor_torque[k] as f64 } else { 0.0 }
            }),
            brake_cmd: co.brake_torque.map(|b| b as f64),
            delta_road: self.driver.steer_road,
            aero_mode: co.aero_mode,
            motor_dead: self.inj.motor_dead,
            mu_scale: self.inj.mu_scale,
        };
        let h = DT_CTRL / PLANT_SUBSTEPS as f64;
        for _ in 0..PLANT_SUBSTEPS {
            self.plant.step(&pin, h);
        }
        // Electrical side
        let p_dc = if hv_on { self.plant.dc_power() + AUX_LOAD_W } else { 0.0 };
        let i = if hv_on {
            match self.battery.current_for_power(p_dc as f32) {
                Some(i) => i as f64,
                None => self.battery.current_for_power(0.95 * self.sop.p_discharge_w).unwrap_or(0.0) as f64,
            }
        } else {
            0.0
        };
        let bo = self.battery.step(i as f32, DT_CTRL as f32, self.t_amb_c as f32, s.vx.abs() as f32);
        self.i_batt = i;
        self.p_batt = bo.p_terminal as f64;
        if self.p_batt > 0.0 {
            self.e_out_j += self.p_batt * DT_CTRL;
        } else {
            self.e_regen_j += -self.p_batt * DT_CTRL;
        }
        self.hv.step(&vo, bo.v_terminal as f64, DT_CTRL);
        for k in 0..4 {
            let t = self.plant.s.t_motor[k] as f32;
            let w = self.plant.s.omega[k] as f32;
            self.motor_th[k].step(&self.p.motor, t, w, 45.0, DT_CTRL as f32);
        }
        self.ekf.step(i as f32, bo.v_terminal, self.battery.s.t_cell_c, DT_CTRL as f32);
        if self.tick % 10 == 0 {
            self.sop = state_of_power(&self.p.battery, &self.battery.s, 10.0);
        }
        self.vsm_out = vo;
        self.vdc_out = co;
        self.t += DT_CTRL;
        self.tick += 1;
        if self.log_every > 0 && self.tick % self.log_every as u64 == 0 {
            self.record();
        }
    }

    pub fn run_for(&mut self, secs: f64) {
        let n = (secs / DT_CTRL).round() as u64;
        for _ in 0..n {
            self.step();
        }
    }

    /// Run until `pred` holds or `timeout` s elapse. Returns elapsed time if reached.
    pub fn run_until(&mut self, timeout: f64, mut pred: impl FnMut(&Sim) -> bool) -> Option<f64> {
        let t0 = self.t;
        while self.t - t0 < timeout {
            self.step();
            if pred(self) {
                return Some(self.t - t0);
            }
        }
        None
    }

    /// Wake, press start with the brake held, select Drive. Returns time to Ready.
    pub fn power_up(&mut self, mode: DriveMode) -> anyhow::Result<f64> {
        self.driver.wake = true;
        self.driver.brake_pedal = 0.3;
        self.driver.mode_request = Some(mode);
        self.run_for(0.01);
        self.driver.start_button = true;
        self.run_for(0.05);
        self.driver.start_button = false;
        let t = self.run_until(2.0, |s| s.vsm.state() == VehicleState::Ready).ok_or_else(|| {
            anyhow::anyhow!("no Ready: state {:?} faults {:?}", self.vsm.state(), self.vsm.faults())
        })?;
        self.driver.gear_request = Some(Gear::Drive);
        self.run_for(0.01);
        self.driver.gear_request = None;
        self.driver.brake_pedal = 0.0;
        self.driver.mode_request = None;
        anyhow::ensure!(self.vsm.state() == VehicleState::Drive, "not in Drive: {:?}", self.vsm.state());
        Ok(t + 0.05)
    }

    fn record(&mut self) {
        let s = &self.plant.s;
        let a = &self.plant.aux;
        let c = &self.vdc_out;
        let v = &self.vsm_out;
        self.log.push(Sample {
            t: self.t,
            x: s.x,
            y: s.y,
            speed_kph: self.plant.speed() * 3.6,
            vx: s.vx,
            vy: s.vy,
            yaw_rate: s.r,
            yaw_rate_ref: c.yaw_rate_ref as f64,
            beta_deg: s.vy.atan2(s.vx.abs().max(1.0)).to_degrees(),
            ax: a.ax,
            ay: a.ay,
            delta_deg: self.driver.steer_road.to_degrees(),
            accel: self.driver.accel_pedal,
            brake: self.driver.brake_pedal,
            state: v.state as u8,
            mode: v.drive_mode as u8,
            aero: c.aero_mode as u8,
            tm_fl: s.t_motor[0],
            tm_fr: s.t_motor[1],
            tm_rl: s.t_motor[2],
            tm_rr: s.t_motor[3],
            tb_fl: s.t_brake[0],
            tb_fr: s.t_brake[1],
            tb_rl: s.t_brake[2],
            tb_rr: s.t_brake[3],
            slip_fl: a.slip_ss[0],
            slip_fr: a.slip_ss[1],
            slip_rl: a.slip_ss[2],
            slip_rr: a.slip_ss[3],
            fz_fl: a.fz[0],
            fz_fr: a.fz[1],
            fz_rl: a.fz[2],
            fz_rr: a.fz[3],
            mz_dem: c.mz_demand as f64,
            mz_ach: c.mz_achieved as f64,
            p_batt_kw: self.p_batt / 1000.0,
            i_batt: self.i_batt,
            v_pack: self.battery.terminal_voltage(self.i_batt as f32) as f64,
            v_link: self.hv.v_link,
            soc: self.battery.s.soc as f64,
            soc_est: self.ekf.soc() as f64,
            t_cell: self.battery.s.t_cell_c as f64,
            t_coolant: self.battery.s.t_coolant_c as f64,
            t_wind_max: self.motor_th.iter().map(|m| m.t_winding_c as f64).fold(f64::MIN, f64::max),
            p_dis_lim_kw: self.sop.p_discharge_w as f64 / 1000.0,
            cd_a: s.cd_a,
            cl_a: s.cl_a,
            downforce: a.downforce,
            main_pos: v.contactors.main_pos,
            torque_enable: v.torque_enable,
        });
    }

    pub fn write_csv(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let mut w = csv::Writer::from_path(path)?;
        for s in &self.log {
            w.serialize(s)?;
        }
        w.flush()?;
        Ok(())
    }

    pub fn aero_mode(&self) -> AeroMode {
        self.vdc_out.aero_mode
    }
}
