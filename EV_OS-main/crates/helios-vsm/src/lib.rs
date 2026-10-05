//! HELIOS Vehicle State Machine (VSM).
//!
//! Runs on the VCU safety MCU (S32K3 lockstep Cortex-M7) at a fixed period.
//! Moore machine: every output is a pure function of (state, phase, latched
//! faults, current inputs), so the safety invariants in `tests/invariants.rs`
//! can be checked against arbitrary input sequences.
//!
//! HV topology assumed (see docs/ARCHITECTURE.md, HV section):
//! pack(+) -- MAIN+ --+-- DC-link (inverters, DC/DC, OBC, e-compressor)
//!         \-- PRE -- R_pc --/
//! pack(-) -- MAIN- -------- DC-link(-)
//! Active discharge resistor switched across the DC-link.
#![no_std]
#![forbid(unsafe_code)]

use helios_core::faults::Reaction;
use helios_core::math::Debounce;
use helios_core::{DriveMode, FaultSet, Gear};
pub use helios_core::Fault;
use helios_core::Fault as Flt;

/// Top-level vehicle states. Values are on the wire (CAN / DDS / VHAL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum VehicleState {
    #[default]
    Sleep = 0,
    Standby = 1,
    Precharge = 2,
    Ready = 3,
    Drive = 4,
    Charging = 5,
    SafeStop = 6,
    HvShutdown = 7,
    Fault = 8,
}

impl VehicleState {
    pub fn from_u8(v: u8) -> Option<Self> {
        use VehicleState::*;
        [Sleep, Standby, Precharge, Ready, Drive, Charging, SafeStop, HvShutdown, Fault].get(v as usize).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShutdownPhase {
    CurrentRamp,
    Discharge,
    Done,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contactors {
    pub main_pos: bool,
    pub main_neg: bool,
    pub precharge: bool,
}

/// Calibration. Construct with [`VsmConfig::for_hv_hardware`] so timing
/// thresholds follow from the actual precharge / discharge RC values.
#[derive(Clone, Copy, Debug)]
pub struct VsmConfig {
    pub period_ms: u32,
    pub pack_v_max: f32,
    pub precharge_ratio: f32,
    pub precharge_min_ms: u32,
    pub precharge_max_ms: u32,
    pub open_current_a: f32,
    pub current_ramp_timeout_ms: u32,
    pub discharge_target_v: f32,
    pub discharge_timeout_ms: u32,
    pub weld_check_ms: u32,
    pub weld_ratio: f32,
    pub iso_fault_ohm_per_v: f32,
    pub iso_warn_ohm_per_v: f32,
    pub hvil_debounce_ms: u32,
    pub iso_debounce_ms: u32,
    pub heartbeat_stale_ms: u32,
    pub heartbeat_lost_ms: u32,
    pub lv_warn_v: f32,
    pub lv_crit_v: f32,
    pub lv_debounce_ms: u32,
    pub safe_stop_decel: f32,
    pub safe_stop_timeout_ms: u32,
    pub standstill_mps: f32,
    pub sleep_delay_ms: u32,
    pub power_off_hold_ms: u32,
    pub pedal_conflict_on: f32,
    pub pedal_conflict_off: f32,
    /// Motor losses tolerated before `MultiMotorLoss` (quad: 1, dual: 0).
    pub tolerated_motor_losses: u8,
}

impl VsmConfig {
    /// Derive precharge and discharge windows from the HV hardware:
    /// the DC-link reaches `ratio` of pack voltage at t = -tau ln(1 - ratio), tau = R_pc C.
    /// Accept [0.5 t, 2.0 t]. Discharge to 60 V from V_max takes tau_d ln(V_max / 60).
    pub fn for_hv_hardware(
        period_ms: u32,
        pack_v_max: f32,
        r_precharge_ohm: f32,
        r_discharge_ohm: f32,
        c_link_f: f32,
        tolerated_motor_losses: u8,
    ) -> Self {
        let ratio = 0.95_f32;
        let tau_pc_ms = r_precharge_ohm * c_link_f * 1000.0;
        let t_pc = -tau_pc_ms * libm::logf(1.0 - ratio);
        let tau_d_ms = r_discharge_ohm * c_link_f * 1000.0;
        let t_dis = tau_d_ms * libm::logf(pack_v_max / 60.0);
        Self {
            period_ms,
            pack_v_max,
            precharge_ratio: ratio,
            precharge_min_ms: (0.5 * t_pc) as u32,
            precharge_max_ms: (2.0 * t_pc) as u32 + period_ms,
            open_current_a: 20.0,
            current_ramp_timeout_ms: 100,
            discharge_target_v: 60.0,
            discharge_timeout_ms: (3.0 * t_dis) as u32,
            weld_check_ms: (1.5 * tau_d_ms) as u32,
            weld_ratio: 0.8,
            iso_fault_ohm_per_v: 500.0,
            iso_warn_ohm_per_v: 1000.0,
            hvil_debounce_ms: 20,
            iso_debounce_ms: 2000,
            heartbeat_stale_ms: 100,
            heartbeat_lost_ms: 500,
            lv_warn_v: 10.5,
            lv_crit_v: 9.0,
            lv_debounce_ms: 50,
            safe_stop_decel: 3.0,
            safe_stop_timeout_ms: 30_000,
            standstill_mps: 0.3,
            sleep_delay_ms: 2000,
            power_off_hold_ms: 2000,
            pedal_conflict_on: 0.25,
            pedal_conflict_off: 0.05,
            tolerated_motor_losses,
        }
    }

    /// HELIOS baseline: 840 V max, 50 ohm precharge, 200 ohm active discharge,
    /// 2.0 mF total DC-link (4 x 500 uF inverters), quad motors, 1 ms period.
    pub fn helios_baseline() -> Self {
        Self::for_hv_hardware(1, 840.0, 50.0, 200.0, 2.0e-3, 1)
    }

    fn ms(&self, ms: u32) -> u32 {
        ms.div_ceil(self.period_ms.max(1))
    }
}

/// Sampled inputs for one VSM step. All analogue values are SI.
#[derive(Clone, Copy, Debug)]
pub struct VsmInputs {
    pub wake: bool,
    pub start_button: bool,
    pub brake_pressed: bool,
    pub accel_pedal: f32,
    pub gear_request: Option<Gear>,
    pub mode_request: Option<DriveMode>,
    pub speed_mps: f32,
    pub pack_voltage: f32,
    pub dc_link_voltage: f32,
    pub pack_current: f32,
    pub hvil_closed: bool,
    pub isolation_ohm: f32,
    pub isolation_valid: bool,
    pub crash: bool,
    pub charge_connector: bool,
    pub charge_request: bool,
    pub bms_critical: bool,
    pub bms_warning: bool,
    pub motor_fault: [bool; 4],
    pub lv_voltage: f32,
    pub heartbeat_age_ms: u32,
    pub thermal_derate: bool,
    pub steering_sensor_fault: bool,
    pub ground_speed_invalid: bool,
    pub service_reset: bool,
}

impl Default for VsmInputs {
    fn default() -> Self {
        Self {
            wake: false,
            start_button: false,
            brake_pressed: false,
            accel_pedal: 0.0,
            gear_request: None,
            mode_request: None,
            speed_mps: 0.0,
            pack_voltage: 760.0,
            dc_link_voltage: 0.0,
            pack_current: 0.0,
            hvil_closed: true,
            isolation_ohm: 5.0e6,
            isolation_valid: true,
            crash: false,
            charge_connector: false,
            charge_request: false,
            bms_critical: false,
            bms_warning: false,
            motor_fault: [false; 4],
            lv_voltage: 13.8,
            heartbeat_age_ms: 0,
            thermal_derate: false,
            steering_sensor_fault: false,
            ground_speed_invalid: false,
            service_reset: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VsmOutputs {
    pub state: VehicleState,
    pub contactors: Contactors,
    pub active_discharge: bool,
    pub torque_enable: bool,
    pub regen_enable: bool,
    pub motor_enable: [bool; 4],
    pub power_scale: f32,
    pub drive_mode: DriveMode,
    pub gear: Gear,
    pub charge_enable: bool,
    /// Deceleration demand to the brake system (SafeStop / HV-off while moving), m/s^2.
    pub decel_request: f32,
    pub faults: FaultSet,
}

#[derive(Debug)]
pub struct Vsm {
    cfg: VsmConfig,
    state: VehicleState,
    t_state: u32,
    phase: ShutdownPhase,
    t_phase: u32,
    precharge_target: VehicleState,
    gear: Gear,
    mode: DriveMode,
    latched: FaultSet,
    active: FaultSet,
    deb_hvil: Debounce,
    deb_iso_fault: Debounce,
    deb_iso_warn: Debounce,
    deb_lv_warn: Debounce,
    deb_lv_crit: Debounce,
    pedal_conflict: bool,
    prev_start: bool,
    start_held: u32,
    not_wake: u32,
    weld_suspect: bool,
    crash_shutdown: bool,
    moving: bool,
    /// Set when HV shutdown begins while moving; keeps the brake-by-wire stop request
    /// alive until standstill even after the VSM has settled in Standby (non-latching
    /// HV-off faults such as HVIL). Found by the twin `hvil` scenario: without it the
    /// car coasted from ~110 km/h once the shutdown sequence completed.
    stop_latch: bool,
}

impl Vsm {
    pub fn new(cfg: VsmConfig) -> Self {
        Self {
            deb_hvil: Debounce::new(cfg.ms(cfg.hvil_debounce_ms)),
            deb_iso_fault: Debounce::new(cfg.ms(cfg.iso_debounce_ms)),
            deb_iso_warn: Debounce::new(cfg.ms(cfg.iso_debounce_ms)),
            deb_lv_warn: Debounce::new(cfg.ms(cfg.lv_debounce_ms)),
            deb_lv_crit: Debounce::new(cfg.ms(cfg.lv_debounce_ms)),
            cfg,
            state: VehicleState::Sleep,
            t_state: 0,
            phase: ShutdownPhase::Done,
            t_phase: 0,
            precharge_target: VehicleState::Ready,
            gear: Gear::Park,
            mode: DriveMode::Road,
            latched: FaultSet::EMPTY,
            active: FaultSet::EMPTY,
            pedal_conflict: false,
            prev_start: false,
            start_held: 0,
            not_wake: 0,
            weld_suspect: false,
            crash_shutdown: false,
            moving: false,
            stop_latch: false,
        }
    }

    pub fn state(&self) -> VehicleState {
        self.state
    }
    pub fn config(&self) -> &VsmConfig {
        &self.cfg
    }
    pub fn faults(&self) -> FaultSet {
        self.active.union(self.latched)
    }
    pub fn latched(&self) -> FaultSet {
        self.latched
    }

    fn goto(&mut self, s: VehicleState) {
        if s != self.state {
            self.state = s;
            self.t_state = 0;
        }
    }

    fn begin_shutdown(&mut self, immediate: bool) {
        // The current-ramp phase keeps MAIN+/MAIN- closed while inverters ramp current to
        // zero. It is only meaningful if the mains were closed in the state we leave;
        // from Precharge (current limited by R_pc) we open immediately. Found by the
        // SSR-VSM-05 property test: aborting a precharge must never close MAIN+.
        let mains_closed = matches!(
            self.state,
            VehicleState::Ready | VehicleState::Drive | VehicleState::Charging | VehicleState::SafeStop
        );
        self.goto(VehicleState::HvShutdown);
        self.phase = if immediate || !mains_closed { ShutdownPhase::Discharge } else { ShutdownPhase::CurrentRamp };
        self.t_phase = 0;
        self.weld_suspect = false;
        self.crash_shutdown = immediate;
        if self.moving {
            self.stop_latch = true;
        }
    }

    fn evaluate_faults(&mut self, i: &VsmInputs) {
        let c = &self.cfg;
        let mut f = FaultSet::EMPTY;
        f.set(Flt::HvilOpen, self.deb_hvil.step(!i.hvil_closed));
        let iso_fault_ohm = c.iso_fault_ohm_per_v * c.pack_v_max;
        let iso_warn_ohm = c.iso_warn_ohm_per_v * c.pack_v_max;
        // `!(x >= thr)` so that a NaN reading with the valid flag set counts as a fault.
        let iso_bad = i.isolation_valid && !(i.isolation_ohm >= iso_fault_ohm);
        let iso_low = i.isolation_valid && !(i.isolation_ohm >= iso_warn_ohm);
        f.set(Flt::IsolationFault, self.deb_iso_fault.step(iso_bad));
        f.set(Flt::IsolationWarning, self.deb_iso_warn.step(iso_low) && !f.contains(Flt::IsolationFault));
        if i.crash {
            self.latched.set(Flt::Crash, true);
        }
        if i.bms_critical {
            self.latched.set(Flt::BmsCritical, true);
        }
        f.set(Flt::BmsWarning, i.bms_warning);
        let motor_bits = [Flt::MotorFaultFL, Flt::MotorFaultFR, Flt::MotorFaultRL, Flt::MotorFaultRR];
        let mut lost = 0u8;
        for (k, mf) in motor_bits.iter().enumerate() {
            f.set(*mf, i.motor_fault[k]);
            lost += i.motor_fault[k] as u8;
        }
        f.set(Flt::MultiMotorLoss, lost > c.tolerated_motor_losses);
        f.set(Flt::CentralHeartbeatLost, i.heartbeat_age_ms > c.heartbeat_lost_ms);
        f.set(
            Flt::CentralHeartbeatStale,
            i.heartbeat_age_ms > c.heartbeat_stale_ms && i.heartbeat_age_ms <= c.heartbeat_lost_ms,
        );
        let lv_crit = self.deb_lv_crit.step(!(i.lv_voltage >= c.lv_crit_v));
        let lv_warn = self.deb_lv_warn.step(!(i.lv_voltage >= c.lv_warn_v));
        f.set(Flt::LvCritical, lv_crit);
        f.set(Flt::LvUndervoltage, lv_warn && !lv_crit);
        f.set(Flt::ChargeConnectorImplausible, i.charge_connector && i.speed_mps.abs() > 1.0);
        // Brake-throttle override with hysteresis (cleared once the accelerator is released).
        if i.brake_pressed && i.accel_pedal > c.pedal_conflict_on {
            self.pedal_conflict = true;
        } else if !(i.accel_pedal >= c.pedal_conflict_off) {
            self.pedal_conflict = false;
        }
        f.set(Flt::PedalConflict, self.pedal_conflict);
        f.set(Flt::ThermalDerate, i.thermal_derate);
        f.set(Flt::SteeringSensorFault, i.steering_sensor_fault);
        f.set(Flt::GroundSpeedInvalid, i.ground_speed_invalid);
        self.active = f;
    }

    /// Conditions required to close the HV contactors from Standby.
    fn hv_permit(&self, i: &VsmInputs) -> bool {
        let all = self.faults();
        self.latched.is_empty()
            && i.hvil_closed
            && !all.contains(Flt::HvilOpen)
            && !all.contains(Flt::IsolationFault)
            && !all.contains(Flt::LvCritical)
            && !all.contains(Flt::MultiMotorLoss)
            && !all.contains(Flt::CentralHeartbeatLost)
            && !i.crash
            && !i.bms_critical
    }

    fn power_scale(&self) -> f32 {
        let f = self.faults();
        let mut s = 1.0_f32;
        let mut take = |cond: bool, v: f32| {
            if cond && v < s {
                s = v;
            }
        };
        take(f.contains(Flt::IsolationFault), 0.3);
        take(f.contains(Flt::BmsWarning), 0.5);
        let motors = [Flt::MotorFaultFL, Flt::MotorFaultFR, Flt::MotorFaultRL, Flt::MotorFaultRR];
        take(motors.iter().any(|m| f.contains(*m)), 0.6);
        take(f.contains(Flt::CentralHeartbeatStale), 0.5);
        take(f.contains(Flt::LvUndervoltage), 0.5);
        take(f.contains(Flt::SteeringSensorFault), 0.3);
        take(f.contains(Flt::GroundSpeedInvalid), 0.7);
        take(f.contains(Flt::ThermalDerate), 0.8);
        take(f.contains(Flt::PedalConflict), 0.0);
        s
    }

    fn handle_gear(&mut self, i: &VsmInputs) {
        let Some(req) = i.gear_request else { return };
        if req == self.gear {
            return;
        }
        let standstill = i.speed_mps.abs() < self.cfg.standstill_mps;
        let ok = match req {
            Gear::Neutral => true,
            Gear::Park => standstill,
            Gear::Drive | Gear::Reverse => {
                // Engaging propulsion needs the brake; direction reversal also needs < 1 m/s.
                let reversing = self.gear.propulsive() && self.gear != req;
                i.brake_pressed && (!reversing || i.speed_mps.abs() < 1.0)
            }
        };
        if ok {
            self.gear = req;
        }
    }

    fn handle_mode(&mut self, i: &VsmInputs) {
        if let Some(m) = i.mode_request {
            if m.driver_selectable() {
                self.mode = m;
            }
        }
    }

    /// Advance one period.
    pub fn step(&mut self, i: &VsmInputs) -> VsmOutputs {
        let c = self.cfg;
        self.t_state = self.t_state.saturating_add(c.period_ms);
        self.t_phase = self.t_phase.saturating_add(c.period_ms);
        self.evaluate_faults(i);
        let start_edge = i.start_button && !self.prev_start;
        self.prev_start = i.start_button;
        self.start_held = if i.start_button { self.start_held.saturating_add(c.period_ms) } else { 0 };
        self.not_wake = if i.wake { 0 } else { self.not_wake.saturating_add(c.period_ms) };
        let standstill = i.speed_mps.abs() < c.standstill_mps;
        self.moving = !standstill;
        if standstill {
            self.stop_latch = false;
        }
        let worst = self.faults().worst();
        let hv_off_now = matches!(worst, Some(Reaction::HvOff));
        let crash = self.latched.contains(Flt::Crash);
        self.handle_mode(i);

        use VehicleState::*;
        match self.state {
            Sleep => {
                if i.wake || i.charge_connector {
                    self.goto(Standby);
                }
            }
            Standby => {
                if self.not_wake >= c.sleep_delay_ms && !i.charge_connector {
                    self.goto(Sleep);
                } else if !self.latched.is_empty() {
                    self.goto(Fault);
                } else if self.hv_permit(i) && standstill {
                    let drive_req = i.wake && start_edge && i.brake_pressed && !self.gear.propulsive();
                    let charge_req = i.charge_connector && i.charge_request;
                    if drive_req || charge_req {
                        self.precharge_target = if drive_req { Ready } else { Charging };
                        self.goto(Precharge);
                    }
                }
            }
            Precharge => {
                let ratio = if i.pack_voltage > 1.0 { i.dc_link_voltage / i.pack_voltage } else { 0.0 };
                if hv_off_now || !self.hv_permit(i) {
                    self.begin_shutdown(crash);
                } else if ratio >= c.precharge_ratio {
                    if self.t_state < c.precharge_min_ms {
                        self.latched.set(Flt::PrechargeTooFast, true);
                        self.begin_shutdown(true);
                    } else {
                        let tgt = self.precharge_target;
                        self.goto(tgt);
                    }
                } else if self.t_state >= c.precharge_max_ms {
                    self.latched.set(Flt::PrechargeTimeout, true);
                    self.begin_shutdown(true);
                }
            }
            Ready => {
                self.handle_gear(i);
                if hv_off_now {
                    self.begin_shutdown(crash);
                } else if matches!(worst, Some(Reaction::SafeStop)) {
                    self.begin_shutdown(false);
                } else if self.gear.propulsive() {
                    self.goto(Drive);
                } else if standstill && i.charge_connector && i.charge_request && self.gear == Gear::Park {
                    self.goto(Charging);
                } else if standstill && (start_edge || self.not_wake >= c.period_ms * 2) {
                    self.gear = Gear::Park;
                    self.begin_shutdown(false);
                }
            }
            Drive => {
                self.handle_gear(i);
                if hv_off_now {
                    self.begin_shutdown(crash);
                } else if matches!(worst, Some(Reaction::SafeStop)) {
                    if standstill {
                        self.gear = Gear::Park;
                        self.begin_shutdown(false);
                    } else {
                        self.goto(SafeStop);
                    }
                } else if !standstill && self.start_held >= c.power_off_hold_ms {
                    // Emergency power-off while moving: controlled stop.
                    self.goto(SafeStop);
                } else if standstill && !self.gear.propulsive() {
                    self.goto(Ready);
                } else if standstill && (start_edge || !i.wake) {
                    self.gear = Gear::Park;
                    self.begin_shutdown(false);
                }
            }
            Charging => {
                if hv_off_now {
                    self.begin_shutdown(crash);
                } else if matches!(worst, Some(Reaction::SafeStop)) || !standstill {
                    self.begin_shutdown(false);
                } else if !i.charge_connector || !i.charge_request {
                    if i.wake {
                        self.goto(Ready);
                    } else {
                        self.begin_shutdown(false);
                    }
                }
            }
            SafeStop => {
                if hv_off_now {
                    self.begin_shutdown(crash);
                } else if standstill || self.t_state >= c.safe_stop_timeout_ms {
                    self.gear = Gear::Park;
                    self.begin_shutdown(false);
                }
            }
            HvShutdown => match self.phase {
                ShutdownPhase::CurrentRamp => {
                    if i.crash {
                        self.crash_shutdown = true;
                    }
                    if self.crash_shutdown
                        || i.pack_current.abs() < c.open_current_a
                        || self.t_phase >= c.current_ramp_timeout_ms
                    {
                        self.phase = ShutdownPhase::Discharge;
                        self.t_phase = 0;
                    }
                }
                ShutdownPhase::Discharge => {
                    if i.dc_link_voltage < c.discharge_target_v {
                        self.phase = ShutdownPhase::Done;
                    } else if self.t_phase >= c.weld_check_ms
                        && i.pack_voltage > 1.0
                        && i.dc_link_voltage > c.weld_ratio * i.pack_voltage
                    {
                        self.latched.set(Flt::ContactorWeld, true);
                        self.weld_suspect = true;
                        self.phase = ShutdownPhase::Done;
                    } else if self.t_phase >= c.discharge_timeout_ms {
                        self.latched.set(Flt::DischargeTimeout, true);
                        self.phase = ShutdownPhase::Done;
                    }
                }
                ShutdownPhase::Done => {
                    if self.latched.is_empty() {
                        self.goto(Standby);
                    } else {
                        self.goto(Fault);
                    }
                }
            },
            Fault => {
                // A service reset clears latched faults only at standstill, with the crash
                // signal gone and no active fault that would immediately re-trip HV.
                let a = self.active;
                let blocking = a.contains(Flt::HvilOpen)
                    || a.contains(Flt::LvCritical)
                    || a.contains(Flt::MultiMotorLoss)
                    || a.contains(Flt::CentralHeartbeatLost);
                if i.service_reset && !i.crash && !i.bms_critical && standstill && !blocking {
                    self.latched = FaultSet::EMPTY;
                    self.weld_suspect = false;
                    self.goto(Standby);
                }
            }
        }
        self.outputs(i)
    }

    fn outputs(&self, i: &VsmInputs) -> VsmOutputs {
        use VehicleState::*;
        let hv_closed = matches!(self.state, Ready | Drive | Charging | SafeStop)
            || (self.state == HvShutdown && self.phase == ShutdownPhase::CurrentRamp);
        let contactors = Contactors {
            main_pos: hv_closed,
            main_neg: hv_closed || self.state == Precharge,
            precharge: self.state == Precharge,
        };
        let weld = self.weld_suspect || self.latched.contains(Flt::ContactorWeld);
        let discharging = !hv_closed
            && self.state != Precharge
            && !weld
            && ((self.state == HvShutdown && self.phase == ShutdownPhase::Discharge)
                || (self.state == Fault && i.dc_link_voltage >= self.cfg.discharge_target_v));
        let charger_present = i.charge_connector && i.speed_mps.abs() <= 1.0;
        let scale = self.power_scale();
        let torque_enable = self.state == Drive && self.gear.propulsive() && !charger_present && scale > 0.0;
        let regen_enable = matches!(self.state, Drive | SafeStop) && !charger_present;
        let motors = [Flt::MotorFaultFL, Flt::MotorFaultFR, Flt::MotorFaultRL, Flt::MotorFaultRR];
        let f = self.faults();
        let motor_enable = core::array::from_fn(|k| (torque_enable || regen_enable) && !f.contains(motors[k]));
        let degraded =
            matches!(f.worst(), Some(Reaction::Degrade) | Some(Reaction::SafeStop) | Some(Reaction::HvOff));
        let decel = if self.moving && (self.stop_latch || matches!(self.state, SafeStop | HvShutdown | Fault)) {
            self.cfg.safe_stop_decel
        } else {
            0.0
        };
        VsmOutputs {
            state: self.state,
            contactors,
            active_discharge: discharging,
            torque_enable,
            regen_enable,
            motor_enable,
            power_scale: if torque_enable { scale } else { 0.0 },
            drive_mode: if degraded { DriveMode::Limp } else { self.mode },
            gear: self.gear,
            charge_enable: self.state == Charging,
            decel_request: decel,
            faults: f,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VsmConfig {
        VsmConfig::helios_baseline()
    }

    #[test]
    fn derived_timing_matches_rc_math() {
        let c = cfg();
        // tau_pc = 50 ohm * 2 mF = 100 ms; t95 = -100 ln(0.05) = 299.57 ms
        assert_eq!(c.precharge_min_ms, 149);
        assert_eq!(c.precharge_max_ms, 600);
        // tau_d = 400 ms; t(840 -> 60 V) = 400 ln(14) = 1055.6 ms; timeout 3x
        assert_eq!(c.discharge_timeout_ms, 3166);
        assert_eq!(c.weld_check_ms, 600);
    }

    /// RC plant for the DC-link used by the unit tests.
    struct HvPlant {
        v_link: f32,
        v_pack: f32,
    }
    impl HvPlant {
        fn step(&mut self, o: &VsmOutputs, dt: f32) {
            let c = 2.0e-3;
            if o.contactors.main_pos && o.contactors.main_neg {
                self.v_link = self.v_pack;
            } else if o.contactors.precharge && o.contactors.main_neg {
                self.v_link += (self.v_pack - self.v_link) * (1.0 - libm::expf(-dt / (50.0 * c)));
            } else if o.active_discharge {
                self.v_link *= libm::expf(-dt / (200.0 * c));
            }
        }
    }

    fn run(vsm: &mut Vsm, plant: &mut HvPlant, i: &mut VsmInputs, ms: u32) -> VsmOutputs {
        let mut o = vsm.step(i);
        for _ in 0..ms {
            i.dc_link_voltage = plant.v_link;
            i.pack_voltage = plant.v_pack;
            o = vsm.step(i);
            plant.step(&o, 0.001);
            i.start_button = false;
        }
        o
    }

    fn to_drive(vsm: &mut Vsm, p: &mut HvPlant, i: &mut VsmInputs) {
        i.wake = true;
        i.brake_pressed = true;
        run(vsm, p, i, 3);
        i.start_button = true;
        run(vsm, p, i, 400);
        assert_eq!(vsm.state(), VehicleState::Ready);
        i.gear_request = Some(Gear::Drive);
        run(vsm, p, i, 2);
        assert_eq!(vsm.state(), VehicleState::Drive);
        i.brake_pressed = false;
    }

    #[test]
    fn nominal_start_drive_stop_sequence() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        let mut i = VsmInputs { wake: true, ..Default::default() };
        run(&mut vsm, &mut p, &mut i, 5);
        assert_eq!(vsm.state(), VehicleState::Standby);
        i.brake_pressed = true;
        i.start_button = true;
        let o = run(&mut vsm, &mut p, &mut i, 2);
        assert_eq!(o.state, VehicleState::Precharge);
        assert!(o.contactors.precharge && o.contactors.main_neg && !o.contactors.main_pos);
        let o = run(&mut vsm, &mut p, &mut i, 400);
        assert_eq!(o.state, VehicleState::Ready, "faults {:?}", o.faults);
        i.gear_request = Some(Gear::Drive);
        let o = run(&mut vsm, &mut p, &mut i, 2);
        assert_eq!(o.state, VehicleState::Drive);
        i.brake_pressed = false;
        let o = run(&mut vsm, &mut p, &mut i, 2);
        assert!(o.torque_enable);
        assert_eq!(o.power_scale, 1.0);
        i.brake_pressed = true;
        i.gear_request = Some(Gear::Park);
        run(&mut vsm, &mut p, &mut i, 2);
        assert_eq!(vsm.state(), VehicleState::Ready);
        i.start_button = true;
        let o = run(&mut vsm, &mut p, &mut i, 1500);
        assert_eq!(o.state, VehicleState::Standby, "faults {:?}", o.faults);
        assert!(p.v_link < 60.0);
    }

    #[test]
    fn precharge_timeout_latches_and_blocks_restart() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        let mut i = VsmInputs { wake: true, brake_pressed: true, ..Default::default() };
        run(&mut vsm, &mut p, &mut i, 3);
        i.start_button = true;
        let mut o = vsm.step(&i);
        for _ in 0..1000 {
            i.dc_link_voltage = 0.0; // open precharge path: link never charges
            o = vsm.step(&i);
        }
        assert!(o.faults.contains(Flt::PrechargeTimeout));
        assert_eq!(o.state, VehicleState::Fault);
        assert!(!o.contactors.main_neg && !o.contactors.precharge);
    }

    #[test]
    fn precharge_too_fast_detects_welded_main_positive() {
        let mut vsm = Vsm::new(cfg());
        let mut i = VsmInputs { wake: true, brake_pressed: true, ..Default::default() };
        vsm.step(&i);
        i.start_button = true;
        i.dc_link_voltage = 760.0; // link already at pack voltage => main+ welded
        let mut o = vsm.step(&i);
        for _ in 0..5 {
            o = vsm.step(&i);
        }
        assert!(o.faults.contains(Flt::PrechargeTooFast));
        assert!(!o.contactors.main_pos);
    }

    #[test]
    fn hvil_open_while_driving_opens_contactors_after_current_ramp() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        let mut i = VsmInputs::default();
        to_drive(&mut vsm, &mut p, &mut i);
        i.speed_mps = 30.0;
        i.pack_current = 400.0;
        run(&mut vsm, &mut p, &mut i, 10);
        i.hvil_closed = false;
        let o = run(&mut vsm, &mut p, &mut i, 18);
        assert!(o.contactors.main_pos, "still debouncing");
        let o = run(&mut vsm, &mut p, &mut i, 2);
        assert_eq!(o.state, VehicleState::HvShutdown);
        assert!(!o.torque_enable);
        assert!(o.decel_request > 0.0);
        let o = run(&mut vsm, &mut p, &mut i, 101);
        assert!(!o.contactors.main_pos && !o.contactors.main_neg);
    }

    #[test]
    fn stop_request_survives_shutdown_completion_until_standstill() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        let mut i = VsmInputs::default();
        to_drive(&mut vsm, &mut p, &mut i);
        i.speed_mps = 40.0;
        i.pack_current = 300.0;
        run(&mut vsm, &mut p, &mut i, 10);
        i.hvil_closed = false;
        run(&mut vsm, &mut p, &mut i, 25);
        i.pack_current = 0.0;
        // Shutdown completes (non-latching fault -> Standby) while still at 40 m/s.
        let o = run(&mut vsm, &mut p, &mut i, 3000);
        assert_eq!(o.state, VehicleState::Standby);
        assert!(o.decel_request > 0.0, "stop request dropped while moving");
        assert!(!o.torque_enable && !o.contactors.main_pos);
        // Restart is inhibited while HVIL is open.
        i.speed_mps = 0.0;
        let o = run(&mut vsm, &mut p, &mut i, 5);
        assert_eq!(o.decel_request, 0.0);
        i.start_button = true;
        i.brake_pressed = true;
        let o = run(&mut vsm, &mut p, &mut i, 5);
        assert_ne!(o.state, VehicleState::Precharge);
    }

    #[test]
    fn crash_opens_contactors_immediately_and_latches() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        let mut i = VsmInputs::default();
        to_drive(&mut vsm, &mut p, &mut i);
        i.speed_mps = 20.0;
        i.pack_current = 900.0;
        i.crash = true;
        let o = vsm.step(&i);
        assert!(!o.contactors.main_pos && !o.contactors.main_neg, "crash must open HV in the same cycle");
        assert!(!o.torque_enable);
        i.crash = false;
        i.speed_mps = 0.0;
        let o = run(&mut vsm, &mut p, &mut i, 2000);
        assert_eq!(o.state, VehicleState::Fault);
        assert!(p.v_link < 60.0, "discharged to {} V", p.v_link);
        i.start_button = true;
        let o = run(&mut vsm, &mut p, &mut i, 10);
        assert_eq!(o.state, VehicleState::Fault);
        i.service_reset = true;
        let o = run(&mut vsm, &mut p, &mut i, 2);
        assert_eq!(o.state, VehicleState::Standby);
    }

    #[test]
    fn single_motor_loss_degrades_double_loss_safe_stops() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        let mut i = VsmInputs::default();
        to_drive(&mut vsm, &mut p, &mut i);
        i.speed_mps = 40.0;
        i.motor_fault[2] = true;
        let o = run(&mut vsm, &mut p, &mut i, 2);
        assert_eq!(o.state, VehicleState::Drive);
        assert_eq!(o.drive_mode, DriveMode::Limp);
        assert!(!o.motor_enable[2] && o.motor_enable[0]);
        assert!((o.power_scale - 0.6).abs() < 1e-6);
        i.motor_fault[3] = true;
        let o = run(&mut vsm, &mut p, &mut i, 2);
        assert_eq!(o.state, VehicleState::SafeStop);
        assert!(!o.torque_enable && o.regen_enable);
        assert!((o.decel_request - 3.0).abs() < 1e-6);
    }

    #[test]
    fn welded_contactor_detected_and_discharge_inhibited() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        let mut i = VsmInputs { wake: true, brake_pressed: true, ..Default::default() };
        run(&mut vsm, &mut p, &mut i, 3);
        i.start_button = true;
        run(&mut vsm, &mut p, &mut i, 400);
        assert_eq!(vsm.state(), VehicleState::Ready);
        i.start_button = true;
        let mut o = vsm.step(&i);
        for _ in 0..2000 {
            i.dc_link_voltage = 760.0; // welded: link tracks pack regardless of command
            o = vsm.step(&i);
        }
        assert!(o.faults.contains(Flt::ContactorWeld));
        assert_eq!(o.state, VehicleState::Fault);
        assert!(!o.active_discharge, "must not burn the discharge resistor against a welded pack");
    }

    #[test]
    fn charging_requires_park_and_blocks_torque() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 700.0 };
        let mut i = VsmInputs { wake: false, charge_connector: true, charge_request: true, ..Default::default() };
        let o = run(&mut vsm, &mut p, &mut i, 400);
        assert_eq!(o.state, VehicleState::Charging);
        assert!(o.charge_enable && !o.torque_enable);
        i.wake = true;
        i.brake_pressed = true;
        i.gear_request = Some(Gear::Drive);
        let o = run(&mut vsm, &mut p, &mut i, 5);
        assert!(!o.torque_enable, "drive-away with charger connected must be impossible");
        i.charge_request = false;
        i.charge_connector = false;
        i.wake = false;
        let o = run(&mut vsm, &mut p, &mut i, 1500);
        assert_eq!(o.state, VehicleState::Standby);
    }

    #[test]
    fn isolation_fault_blocks_hv_closure() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        // 300 kOhm < 500 ohm/V * 840 V = 420 kOhm
        let mut i = VsmInputs { wake: true, brake_pressed: true, isolation_ohm: 300e3, ..Default::default() };
        run(&mut vsm, &mut p, &mut i, 2100);
        assert!(vsm.faults().contains(Flt::IsolationFault));
        i.start_button = true;
        let o = run(&mut vsm, &mut p, &mut i, 50);
        assert_eq!(o.state, VehicleState::Standby);
        assert!(!o.contactors.main_neg);
    }

    #[test]
    fn brake_override_cuts_propulsion() {
        let mut vsm = Vsm::new(cfg());
        let mut p = HvPlant { v_link: 0.0, v_pack: 760.0 };
        let mut i = VsmInputs::default();
        to_drive(&mut vsm, &mut p, &mut i);
        i.speed_mps = 25.0;
        i.accel_pedal = 0.8;
        let o = run(&mut vsm, &mut p, &mut i, 2);
        assert!(o.torque_enable);
        i.brake_pressed = true;
        let o = run(&mut vsm, &mut p, &mut i, 1);
        assert!(!o.torque_enable, "brake must override accelerator");
        i.brake_pressed = false;
        let o = run(&mut vsm, &mut p, &mut i, 1);
        assert!(!o.torque_enable, "stays cut until accelerator released (hysteresis)");
        i.accel_pedal = 0.0;
        let o = run(&mut vsm, &mut p, &mut i, 1);
        assert!(o.torque_enable);
    }
}
