//! HELIOS baseline vehicle parameters.
//!
//! Sources (see docs/ARCHITECTURE.md section 6 for the derivations):
//! * Tyres: Pirelli P Zero Trofeo RS 305/30ZR21 F, 335/30ZR21 R (project BOM
//!   alternate set; 21 in rims at all corners are required by the Donut Lab
//!   21 in in-wheel motor).
//! * Steering: DCE dual-MGU EPAS rack, 2.5 turns lock-to-lock.
//! * Motor: Donut Lab 21 in in-wheel motor, 630 kW / 4,300 Nm peak, 40 kg.
//! * Mass, CG, inertia, aero maps: design-point estimates pending CAD/CFD.

use crate::consts::{DEG, G, RHO_AIR_ISA};
use crate::math::{atan2, sqrt};
use crate::modes::AeroMode;
use crate::{Corner, PerCorner};

/// ISO metric tyre size, e.g. 305/30 R21.
#[derive(Clone, Copy, Debug)]
pub struct TyreSize {
    pub section_width_mm: f32,
    pub aspect_ratio_pct: f32,
    pub rim_diameter_in: f32,
}

impl TyreSize {
    /// Unloaded outer radius, m.
    pub fn free_radius(&self) -> f32 {
        let sidewall = self.section_width_mm * self.aspect_ratio_pct / 100.0;
        (self.rim_diameter_in * 25.4 + 2.0 * sidewall) / 2000.0
    }
    /// Effective rolling radius, m. Low-profile race tyres run ~2.5 % below free radius.
    pub fn rolling_radius(&self) -> f32 {
        0.975 * self.free_radius()
    }
}

/// Simplified Magic Formula coefficients (pure slip), normalised by mu*Fz.
#[derive(Clone, Copy, Debug)]
pub struct TyreMf {
    pub bx: f32,
    pub cx: f32,
    pub ex: f32,
    pub by: f32,
    pub cy: f32,
    pub ey: f32,
    /// Peak friction at nominal load Fz0.
    pub mu0: f32,
    /// Nominal load, N.
    pub fz0: f32,
    /// Load sensitivity: mu = mu0 * (1 + pdx2 * dfz), dfz = (Fz - Fz0)/Fz0.
    pub pdx2: f32,
    /// Relaxation lengths, m.
    pub sigma_x: f32,
    pub sigma_y: f32,
    /// Rolling resistance coefficient.
    pub crr: f32,
}

impl TyreMf {
    /// Slip ratio at which pure longitudinal force peaks (E = 0 closed form):
    /// C*atan(B*k) = pi/2  =>  k = tan(pi/(2C))/B.
    pub fn kappa_peak(&self) -> f32 {
        libm::tanf(core::f32::consts::FRAC_PI_2 / self.cx) / self.bx
    }
    pub fn alpha_peak(&self) -> f32 {
        libm::tanf(core::f32::consts::FRAC_PI_2 / self.cy) / self.by
    }
    pub fn mu(&self, fz: f32) -> f32 {
        let dfz = (fz - self.fz0) / self.fz0;
        self.mu0 * (1.0 + self.pdx2 * dfz)
    }
    /// Linear cornering stiffness at load fz, N/rad: dFy/dalpha|0 = B*C*D.
    pub fn cornering_stiffness(&self, fz: f32) -> f32 {
        self.by * self.cy * self.mu(fz) * fz
    }
}

/// Peak motor envelope (per corner).
#[derive(Clone, Copy, Debug)]
pub struct MotorSpec {
    pub peak_torque_nm: f32,
    pub peak_power_w: f32,
    pub max_speed_rad_s: f32,
    /// Regen torque/power as a fraction of drive peak.
    pub regen_fraction: f32,
    /// Electrical time constant of the torque loop (inverter + current control), s.
    pub torque_tau_s: f32,
    /// Efficiency at the corner point (peak torque, base speed) used to size copper loss.
    pub eta_corner: f32,
    /// Iron + windage loss at max speed, zero torque, W.
    pub iron_loss_at_max_w: f32,
    /// Winding thermal capacity, J/K, and conductance to coolant, W/K.
    pub winding_c_j_k: f32,
    pub winding_ua_w_k: f32,
    /// Derate window on winding temperature, degC.
    pub derate_start_c: f32,
    pub derate_end_c: f32,
    pub mass_kg: f32,
}

impl MotorSpec {
    pub fn base_speed(&self) -> f32 {
        self.peak_power_w / self.peak_torque_nm
    }
    /// Max drive torque at speed (constant torque / constant power).
    pub fn torque_limit(&self, omega: f32) -> f32 {
        let w = libm::fabsf(omega);
        if w >= self.max_speed_rad_s {
            return 0.0;
        }
        let tp = if w > 1e-3 { self.peak_power_w / w } else { f32::INFINITY };
        if tp < self.peak_torque_nm { tp } else { self.peak_torque_nm }
    }
    pub fn regen_torque_limit(&self, omega: f32) -> f32 {
        self.regen_fraction * self.torque_limit(omega)
    }
    /// Copper-loss coefficient c_cu [W/Nm^2] such that at the corner point
    /// losses = P(1/eta - 1), minus the iron share at base speed.
    pub fn copper_coeff(&self) -> f32 {
        let wb = self.base_speed();
        let p_loss_corner = self.peak_power_w * (1.0 / self.eta_corner - 1.0);
        let iron_at_base = self.iron_coeff() * wb * wb;
        (p_loss_corner - iron_at_base) / (self.peak_torque_nm * self.peak_torque_nm)
    }
    /// Iron/windage coefficient c_fe [W s^2/rad^2].
    pub fn iron_coeff(&self) -> f32 {
        self.iron_loss_at_max_w / (self.max_speed_rad_s * self.max_speed_rad_s)
    }
    /// Total motor loss at (T, omega), W.
    pub fn loss(&self, torque: f32, omega: f32) -> f32 {
        self.copper_coeff() * torque * torque + self.iron_coeff() * omega * omega
    }
}

/// Aero coefficients for one active-aero configuration.
#[derive(Clone, Copy, Debug)]
pub struct AeroPoint {
    /// Drag area Cd*A, m^2.
    pub cd_a: f32,
    /// Downforce area Cl*A (positive = downforce), m^2.
    pub cl_a: f32,
    /// Fraction of downforce on the front axle.
    pub balance_front: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct AeroMap {
    pub low_drag: AeroPoint,
    pub high_downforce: AeroPoint,
    pub airbrake: AeroPoint,
    /// Rear-wing actuator full-travel time, s (sets mode transition rate).
    pub transition_time_s: f32,
}

impl AeroMap {
    pub fn point(&self, m: AeroMode) -> AeroPoint {
        match m {
            AeroMode::LowDrag => self.low_drag,
            AeroMode::HighDownforce => self.high_downforce,
            AeroMode::Airbrake => self.airbrake,
        }
    }
    /// Downforce N at speed v (m/s), ISA density.
    pub fn downforce(&self, m: AeroMode, v: f32) -> f32 {
        0.5 * RHO_AIR_ISA * self.point(m).cl_a * v * v
    }
    pub fn drag(&self, m: AeroMode, v: f32) -> f32 {
        0.5 * RHO_AIR_ISA * self.point(m).cd_a * v * libm::fabsf(v)
    }
}

/// Battery pack (series x parallel of identical cells) with a 2-RC equivalent circuit.
#[derive(Clone, Copy, Debug)]
pub struct BatterySpec {
    pub series: u16,
    pub parallel: u16,
    pub cell_capacity_ah: f32,
    pub cell_v_min: f32,
    pub cell_v_max: f32,
    pub cell_r0_ohm: f32,
    pub cell_r1_ohm: f32,
    pub cell_tau1_s: f32,
    pub cell_r2_ohm: f32,
    pub cell_tau2_s: f32,
    /// Arrhenius activation temperature Ea/R for resistance, K.
    pub r_arrhenius_k: f32,
    pub cell_mass_kg: f32,
    pub cell_cp_j_kgk: f32,
    /// Cell-to-coolant conductance for the whole pack, W/K.
    pub pack_ua_w_k: f32,
    pub max_discharge_c: f32,
    pub max_charge_c: f32,
    pub t_max_c: f32,
    pub t_derate_start_c: f32,
}

impl BatterySpec {
    pub fn pack_capacity_ah(&self) -> f32 {
        self.cell_capacity_ah * self.parallel as f32
    }
    pub fn n_cells(&self) -> u32 {
        self.series as u32 * self.parallel as u32
    }
    pub fn r0_pack(&self, t_cell_c: f32) -> f32 {
        self.cell_r0_ohm * self.series as f32 / self.parallel as f32 * self.r_temp_factor(t_cell_c)
    }
    /// Arrhenius scaling referenced to 25 degC.
    pub fn r_temp_factor(&self, t_cell_c: f32) -> f32 {
        let t = t_cell_c + 273.15;
        let tr = 298.15;
        libm::expf(self.r_arrhenius_k * (1.0 / t - 1.0 / tr))
    }
    pub fn max_discharge_current(&self) -> f32 {
        self.max_discharge_c * self.pack_capacity_ah()
    }
    pub fn max_charge_current(&self) -> f32 {
        self.max_charge_c * self.pack_capacity_ah()
    }
    pub fn thermal_mass_j_k(&self) -> f32 {
        self.n_cells() as f32 * self.cell_mass_kg * self.cell_cp_j_kgk
    }
    pub fn v_max_pack(&self) -> f32 {
        self.series as f32 * self.cell_v_max
    }
    pub fn v_min_pack(&self) -> f32 {
        self.series as f32 * self.cell_v_min
    }
}

/// NMC open-circuit voltage vs SOC (cell, 25 degC), 11 points at 0.1 SOC spacing.
/// Representative high-power NMC pouch data; replace with the supplier's HPPC table.
pub const OCV_SOC: [f32; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
pub const OCV_V: [f32; 11] = [3.00, 3.45, 3.56, 3.63, 3.68, 3.74, 3.81, 3.90, 3.99, 4.08, 4.20];

pub fn cell_ocv(soc: f32) -> f32 {
    crate::math::interp1(&OCV_SOC, &OCV_V, soc)
}

/// Which corners carry a traction motor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowertrainLayout {
    /// One in-wheel motor per corner: full per-wheel torque vectoring.
    Quad,
    /// Two rear in-wheel motors: rear left/right vectoring, front friction-only.
    DualRear,
    /// One motor per axle through an open differential: no left/right vectoring
    /// by drive torque (brake-based only).
    DualAxle,
}

impl PowertrainLayout {
    pub const fn driven(self, c: Corner) -> bool {
        match self {
            PowertrainLayout::Quad | PowertrainLayout::DualAxle => true,
            PowertrainLayout::DualRear => !c.is_front(),
        }
    }
    pub const fn independent_lr(self) -> bool {
        !matches!(self, PowertrainLayout::DualAxle)
    }
    pub const fn motor_count(self) -> usize {
        match self {
            PowertrainLayout::Quad => 4,
            _ => 2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VehicleParams {
    /// Test mass incl. 75 kg driver, kg.
    pub mass_kg: f32,
    pub wheelbase_m: f32,
    /// Static rear-axle load fraction (=> a = frac_rear * L).
    pub weight_frac_rear: f32,
    pub cg_height_m: f32,
    pub track_front_m: f32,
    pub track_rear_m: f32,
    pub yaw_inertia_kgm2: f32,
    /// Front share of total roll stiffness.
    pub roll_stiffness_front: f32,
    pub tyre_front: TyreSize,
    pub tyre_rear: TyreSize,
    pub tyre_mf: TyreMf,
    /// Rotating inertia per corner (tyre + rim + motor rotor + disc), kg m^2.
    pub wheel_inertia_kgm2: f32,
    pub steering_ratio: f32,
    pub max_road_wheel_angle: f32,
    pub layout: PowertrainLayout,
    pub motor: MotorSpec,
    pub inverter_eta: f32,
    pub battery: BatterySpec,
    pub aero: AeroMap,
    /// Max friction brake torque per corner, Nm.
    pub brake_torque_max_front: f32,
    pub brake_torque_max_rear: f32,
    /// System-level peak drive power (battery-limited), W.
    pub system_peak_power_w: f32,
}

impl VehicleParams {
    pub fn a(&self) -> f32 {
        self.weight_frac_rear * self.wheelbase_m
    }
    pub fn b(&self) -> f32 {
        self.wheelbase_m - self.a()
    }
    pub fn rolling_radius(&self, c: Corner) -> f32 {
        if c.is_front() { self.tyre_front.rolling_radius() } else { self.tyre_rear.rolling_radius() }
    }
    pub fn radii(&self) -> PerCorner<f32> {
        Corner::ALL.map(|c| self.rolling_radius(c))
    }
    /// Contact patch position in the body frame (x forward, y left), m.
    pub fn corner_xy(&self, c: Corner) -> (f32, f32) {
        let x = if c.is_front() { self.a() } else { -self.b() };
        let t = if c.is_front() { self.track_front_m } else { self.track_rear_m };
        (x, c.y_sign() * t / 2.0)
    }
    /// Static vertical load per corner, N.
    pub fn static_fz(&self, c: Corner) -> f32 {
        let l = self.wheelbase_m;
        let axle = if c.is_front() { self.b() / l } else { self.a() / l };
        self.mass_kg * G * axle / 2.0
    }
    /// Quasi-static vertical loads from longitudinal/lateral acceleration and aero.
    /// ax > 0 accelerating, ay > 0 = left turn (load moves to the right wheels).
    pub fn quasi_static_fz(&self, ax: f32, ay: f32, v: f32, aero: AeroMode) -> PerCorner<f32> {
        let l = self.wheelbase_m;
        let m = self.mass_kg;
        let h = self.cg_height_m;
        let df = self.aero.downforce(aero, v);
        let bal = self.aero.point(aero).balance_front;
        let d_long = m * ax * h / l; // moved from front axle to rear axle
        let d_lat_f = m * ay * h * self.roll_stiffness_front / self.track_front_m;
        let d_lat_r = m * ay * h * (1.0 - self.roll_stiffness_front) / self.track_rear_m;
        Corner::ALL.map(|c| {
            let base = self.static_fz(c);
            let aero_c = if c.is_front() { df * bal / 2.0 } else { df * (1.0 - bal) / 2.0 };
            let long = if c.is_front() { -d_long / 2.0 } else { d_long / 2.0 };
            let lat = if c.is_front() { d_lat_f } else { d_lat_r };
            let lat = if c.is_left() { -lat } else { lat };
            let fz = base + aero_c + long + lat;
            if fz > 0.0 { fz } else { 0.0 }
        })
    }
    /// Kinematic road-wheel angles (Ackermann) for a mean front angle delta.
    /// Positive delta = left turn. Rear wheels unsteered.
    pub fn ackermann(&self, delta: f32) -> PerCorner<f32> {
        if libm::fabsf(delta) < 1e-6 {
            return [0.0; 4];
        }
        let l = self.wheelbase_m;
        let r = l / libm::tanf(delta); // turn radius to rear-axle centre, signed
        let half = self.track_front_m / 2.0;
        let fl = atan2(l, r - half);
        let fr = atan2(l, r + half);
        // atan2 returns angle in (0, pi) for negative r; fold back to (-pi/2, pi/2).
        let fold = |x: f32| if x > core::f32::consts::FRAC_PI_2 { x - core::f32::consts::PI } else { x };
        [fold(fl), fold(fr), 0.0, 0.0]
    }
    /// Speed above which the vehicle is power-limited rather than torque-limited.
    pub fn system_base_speed(&self) -> f32 {
        let n = self.layout.motor_count() as f32;
        let r = self.tyre_rear.rolling_radius();
        self.system_peak_power_w / (n * self.motor.peak_torque_nm / r)
    }
    /// Lateral-acceleration limit for yaw reference saturation, m/s^2.
    pub fn ay_limit(&self, mu: f32, v: f32, aero: AeroMode) -> f32 {
        let df = self.aero.downforce(aero, v);
        mu * (self.mass_kg * G + df) / self.mass_kg
    }
    /// Characteristic understeer gradient of the passive vehicle from linear
    /// cornering stiffness at static load, rad/(m/s^2): K = m/L (b/Cf - a/Cr)... sign per Milliken:
    /// K = (m/L) * (b/C_f - a/C_r) with axle stiffness C = 2 * C_alpha.
    pub fn passive_understeer_gradient(&self) -> f32 {
        let cf = 2.0 * self.tyre_mf.cornering_stiffness(self.static_fz(Corner::FrontLeft));
        let cr = 2.0 * self.tyre_mf.cornering_stiffness(self.static_fz(Corner::RearLeft));
        self.mass_kg / self.wheelbase_m * (self.b() / cf - self.a() / cr)
    }
    pub fn hypot(x: f32, y: f32) -> f32 {
        sqrt(x * x + y * y)
    }
}

pub const TROFEO_RS_FRONT: TyreSize =
    TyreSize { section_width_mm: 305.0, aspect_ratio_pct: 30.0, rim_diameter_in: 21.0 };
pub const TROFEO_RS_REAR: TyreSize =
    TyreSize { section_width_mm: 335.0, aspect_ratio_pct: 30.0, rim_diameter_in: 21.0 };

/// Design-point baseline (quad-motor). All values SI.
pub const HELIOS_BASELINE: VehicleParams = VehicleParams {
    mass_kg: 1500.0,
    wheelbase_m: 2.70,
    weight_frac_rear: 0.55,
    cg_height_m: 0.40,
    track_front_m: 1.68,
    track_rear_m: 1.64,
    yaw_inertia_kgm2: 2571.0,
    roll_stiffness_front: 0.52,
    tyre_front: TROFEO_RS_FRONT,
    tyre_rear: TROFEO_RS_REAR,
    tyre_mf: TyreMf {
        bx: 14.0,
        cx: 1.65,
        ex: 0.0,
        by: 14.0,
        cy: 1.45,
        ey: 0.0,
        mu0: 1.50,
        fz0: 4000.0,
        pdx2: -0.08,
        sigma_x: 0.12,
        sigma_y: 0.35,
        crr: 0.012,
    },
    wheel_inertia_kgm2: 2.9,
    steering_ratio: 15.0,
    max_road_wheel_angle: 30.0 * DEG,
    layout: PowertrainLayout::Quad,
    motor: MotorSpec {
        peak_torque_nm: 4300.0,
        peak_power_w: 630_000.0,
        max_speed_rad_s: 2800.0 * core::f32::consts::PI / 30.0,
        regen_fraction: 0.8,
        torque_tau_s: 0.002,
        eta_corner: 0.93,
        iron_loss_at_max_w: 3000.0,
        winding_c_j_k: 6000.0,
        winding_ua_w_k: 250.0,
        derate_start_c: 150.0,
        derate_end_c: 180.0,
        mass_kg: 40.0,
    },
    inverter_eta: 0.985,
    battery: BatterySpec {
        series: 200,
        parallel: 3,
        cell_capacity_ah: 45.0,
        cell_v_min: 2.8,
        cell_v_max: 4.2,
        cell_r0_ohm: 0.000_45,
        cell_r1_ohm: 0.000_25,
        cell_tau1_s: 10.0,
        cell_r2_ohm: 0.000_20,
        cell_tau2_s: 100.0,
        r_arrhenius_k: 2800.0,
        cell_mass_kg: 0.64,
        cell_cp_j_kgk: 1050.0,
        pack_ua_w_k: 1800.0,
        max_discharge_c: 16.0,
        max_charge_c: 8.0,
        t_max_c: 60.0,
        t_derate_start_c: 50.0,
    },
    aero: AeroMap {
        low_drag: AeroPoint { cd_a: 0.585, cl_a: 0.45, balance_front: 0.40 },
        high_downforce: AeroPoint { cd_a: 0.82, cl_a: 2.40, balance_front: 0.42 },
        airbrake: AeroPoint { cd_a: 1.27, cl_a: 2.90, balance_front: 0.35 },
        transition_time_s: 0.35,
    },
    brake_torque_max_front: 6500.0,
    brake_torque_max_rear: 4500.0,
    system_peak_power_w: 1_500_000.0,
};

impl VehicleParams {
    /// Same vehicle with a different powertrain layout. Dual layouts reuse the
    /// 630 kW motor; system power drops to 2 x 630 kW.
    pub fn with_layout(mut self, layout: PowertrainLayout) -> Self {
        self.layout = layout;
        if layout != PowertrainLayout::Quad {
            self.system_peak_power_w = 2.0 * self.motor.peak_power_w;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tyre_radii_from_iso_size() {
        // 305/30R21: sidewall 91.5 mm, D = 533.4 + 183 = 716.4 mm => r = 358.2 mm
        assert!((TROFEO_RS_FRONT.free_radius() - 0.3582).abs() < 1e-4);
        // 335/30R21: sidewall 100.5 mm, D = 734.4 mm => r = 367.2 mm
        assert!((TROFEO_RS_REAR.free_radius() - 0.3672).abs() < 1e-4);
    }

    #[test]
    fn static_loads_sum_to_weight() {
        let p = HELIOS_BASELINE;
        let s: f32 = Corner::ALL.iter().map(|c| p.static_fz(*c)).sum();
        assert!((s - p.mass_kg * G).abs() < 0.5);
        let rear = p.static_fz(Corner::RearLeft) * 2.0 / (p.mass_kg * G);
        assert!((rear - 0.55).abs() < 1e-4);
    }

    #[test]
    fn quasi_static_loads_conserve_force() {
        let p = HELIOS_BASELINE;
        let v = 60.0;
        let fz = p.quasi_static_fz(5.0, 8.0, v, AeroMode::HighDownforce);
        let total: f32 = fz.iter().sum();
        let expected = p.mass_kg * G + p.aero.downforce(AeroMode::HighDownforce, v);
        assert!((total - expected).abs() / expected < 1e-4);
        // left turn => right wheels heavier
        assert!(fz[Corner::FrontRight.idx()] > fz[Corner::FrontLeft.idx()]);
    }

    #[test]
    fn motor_envelope_corner_point() {
        let m = HELIOS_BASELINE.motor;
        let wb = m.base_speed();
        assert!((wb - 146.51).abs() < 0.01, "base speed {wb}");
        assert!((m.torque_limit(wb * 2.0) * wb * 2.0 - m.peak_power_w).abs() < 1.0);
        let loss = m.loss(m.peak_torque_nm, wb);
        let expected = m.peak_power_w * (1.0 / m.eta_corner - 1.0);
        assert!((loss - expected).abs() / expected < 1e-4);
    }

    #[test]
    fn steering_ratio_matches_rack_spec() {
        // 2.5 turns lock-to-lock = 900 deg handwheel, +/-30 deg road wheel => 15:1
        let p = HELIOS_BASELINE;
        let ratio = (2.5 * 360.0 / 2.0) * DEG / p.max_road_wheel_angle;
        assert!((ratio - p.steering_ratio).abs() < 1e-3);
    }

    #[test]
    fn ackermann_inner_wheel_steers_more() {
        let p = HELIOS_BASELINE;
        let d = p.ackermann(10.0 * DEG);
        assert!(d[0] > d[1] && d[1] > 0.0, "{d:?}");
        let d = p.ackermann(-10.0 * DEG);
        assert!(d[1] < d[0] && d[0] < 0.0, "{d:?}");
    }

    #[test]
    fn pack_is_800v_class() {
        let b = HELIOS_BASELINE.battery;
        assert!((b.v_max_pack() - 840.0).abs() < 1e-3);
        let e_kwh = b.series as f32 * b.pack_capacity_ah() * 3.7 / 1000.0;
        assert!((e_kwh - 99.9).abs() < 0.1);
    }
}
