//! Safety invariants of the VSM checked over random input sequences.
//!
//! Each case drives the VSM with up to 3,000 random-but-structured input
//! samples (1 ms each) against an RC model of the DC-link, and checks the
//! invariants at every step. These are the software safety requirements
//! SSR-VSM-01..09 in docs/ARCHITECTURE.md.

use helios_core::{Fault, Gear};
use helios_vsm::{VehicleState, Vsm, VsmConfig, VsmInputs, VsmOutputs};
use proptest::prelude::*;

#[derive(Clone, Debug)]
struct Sample {
    hold_ms: u16,
    wake: bool,
    start: bool,
    brake: bool,
    accel: f32,
    gear: Option<Gear>,
    speed: f32,
    current: f32,
    hvil: bool,
    iso_kohm: f32,
    crash: bool,
    connector: bool,
    charge_req: bool,
    bms_crit: bool,
    motor_fault: [bool; 4],
    lv: f32,
    hb_ms: u32,
    reset: bool,
    weld: bool,
    precharge_broken: bool,
}

fn gear() -> impl Strategy<Value = Option<Gear>> {
    prop_oneof![
        4 => Just(None),
        1 => Just(Some(Gear::Park)),
        1 => Just(Some(Gear::Neutral)),
        2 => Just(Some(Gear::Drive)),
        1 => Just(Some(Gear::Reverse)),
    ]
}

fn sample() -> impl Strategy<Value = Sample> {
    (
        (prop_oneof![9 => 1u16..400, 1 => 1500u16..3000], prop::bool::weighted(0.9), prop::bool::weighted(0.2), any::<bool>(), 0.0f32..1.0, gear()),
        (prop_oneof![3 => Just(0.0f32), 2 => 0.0f32..80.0], -200.0f32..1500.0, prop::bool::weighted(0.97)),
        (prop_oneof![9 => Just(5000.0f32), 1 => 100.0f32..1000.0], prop::bool::weighted(0.02)),
        (prop::bool::weighted(0.1), any::<bool>(), prop::bool::weighted(0.02)),
        (prop::array::uniform4(prop::bool::weighted(0.05)), prop_oneof![9 => Just(13.8f32), 1 => 8.0f32..14.0]),
        (prop_oneof![9 => Just(0u32), 1 => 0u32..1000], prop::bool::weighted(0.05)),
        (prop::bool::weighted(0.02), prop::bool::weighted(0.03)),
    )
        .prop_map(|(a, b, c, d, e, f, g)| Sample {
            hold_ms: a.0,
            wake: a.1,
            start: a.2,
            brake: a.3,
            accel: a.4,
            gear: a.5,
            speed: b.0,
            current: b.1,
            hvil: b.2,
            iso_kohm: c.0,
            crash: c.1,
            connector: d.0,
            charge_req: d.1,
            bms_crit: d.2,
            motor_fault: e.0,
            lv: e.1,
            hb_ms: f.0,
            reset: f.1,
            weld: g.0,
            precharge_broken: g.1,
        })
}

struct Plant {
    v_link: f32,
    v_pack: f32,
}

impl Plant {
    fn step(&mut self, o: &VsmOutputs, s: &Sample) {
        let dt: f32 = 0.001;
        let c: f32 = 2.0e-3;
        if s.weld {
            self.v_link = self.v_pack;
        } else if o.contactors.main_pos && o.contactors.main_neg {
            self.v_link = self.v_pack;
        } else if o.contactors.precharge && o.contactors.main_neg && !s.precharge_broken {
            self.v_link += (self.v_pack - self.v_link) * (1.0 - (-dt / (50.0 * c)).exp());
        } else if o.active_discharge {
            self.v_link *= (-dt / (200.0 * c)).exp();
        } else {
            self.v_link *= (-dt / (50_000.0 * c)).exp(); // passive bleed
        }
    }
}

fn check(o: &VsmOutputs, prev: &VsmOutputs, i: &VsmInputs, crash_seen: bool) -> Result<(), TestCaseError> {
    let k = &o.contactors;
    // SSR-VSM-01: propulsion only in Drive with a propulsive gear and closed HV.
    if o.torque_enable {
        prop_assert_eq!(o.state, VehicleState::Drive);
        prop_assert!(o.gear.propulsive());
        prop_assert!(k.main_pos && k.main_neg);
    }
    // SSR-VSM-02: the active discharge resistor is never across a closed pack.
    if o.active_discharge {
        prop_assert!(!k.main_pos && !k.main_neg && !k.precharge);
    }
    // SSR-VSM-03: precharge relay only in Precharge, only with MAIN- closed and MAIN+ open.
    if k.precharge {
        prop_assert_eq!(o.state, VehicleState::Precharge);
        prop_assert!(k.main_neg && !k.main_pos);
    }
    // SSR-VSM-04: MAIN+ never closed without MAIN-.
    if k.main_pos {
        prop_assert!(k.main_neg);
    }
    // SSR-VSM-05: MAIN+ only closes after a completed precharge (link >= 95 % pack).
    if k.main_pos && !prev.contactors.main_pos {
        prop_assert_eq!(prev.state, VehicleState::Precharge);
        prop_assert!(i.dc_link_voltage >= 0.95 * i.pack_voltage);
    }
    // SSR-VSM-06: after a crash no propulsion and HV open, until a service reset.
    if crash_seen {
        prop_assert!(!o.torque_enable);
        prop_assert!(!k.main_pos && !k.precharge);
    }
    // SSR-VSM-07: charging and propulsion are mutually exclusive.
    prop_assert!(!(o.charge_enable && o.torque_enable));
    // SSR-VSM-08: no propulsion with a charge connector detected at (near) standstill.
    if i.charge_connector && i.speed_mps.abs() <= 1.0 {
        prop_assert!(!o.torque_enable);
    }
    // SSR-VSM-09: brake + accelerator conflict always removes propulsion.
    if i.brake_pressed && i.accel_pedal > 0.25 {
        prop_assert!(!o.torque_enable);
    }
    Ok(())
}

#[derive(Default)]
struct Coverage {
    states: [u64; 9],
    faults: u64,
}

fn simulate(seq: &[Sample], cov: &mut Coverage) -> Result<(), TestCaseError> {
    let mut vsm = Vsm::new(VsmConfig::helios_baseline());
    let mut plant = Plant { v_link: 0.0, v_pack: 760.0 };
    let mut i = VsmInputs::default();
    let mut prev = vsm.step(&i);
    let mut crash_seen = false;
    let mut hvil_open_ms = 0u32;
    for s in seq {
        for t in 0..s.hold_ms {
            i.wake = s.wake;
            i.start_button = s.start && t < 50;
            i.brake_pressed = s.brake;
            i.accel_pedal = s.accel;
            i.gear_request = s.gear;
            i.speed_mps = s.speed;
            i.pack_current = if prev.contactors.main_pos { s.current } else { 0.0 };
            i.hvil_closed = s.hvil;
            i.isolation_ohm = s.iso_kohm * 1e3;
            i.crash = s.crash;
            i.charge_connector = s.connector;
            i.charge_request = s.charge_req;
            i.bms_critical = s.bms_crit;
            i.motor_fault = s.motor_fault;
            i.lv_voltage = s.lv;
            i.heartbeat_age_ms = s.hb_ms;
            i.service_reset = s.reset;
            i.pack_voltage = plant.v_pack;
            i.dc_link_voltage = plant.v_link;
            let o = vsm.step(&i);
            cov.states[o.state as usize] += 1;
            cov.faults |= o.faults.0;
            if s.crash {
                crash_seen = true;
            }
            check(&o, &prev, &i, crash_seen)?;
            if s.reset && o.state == VehicleState::Standby {
                crash_seen = false;
            }
            // SSR-VSM-10: HVIL open => HV open within debounce (20) + current ramp (100) + 2 ms.
            hvil_open_ms = if i.hvil_closed { 0 } else { hvil_open_ms + 1 };
            if hvil_open_ms > 122 {
                prop_assert!(!o.contactors.main_pos, "HV still closed {} ms after HVIL open", hvil_open_ms);
            }
            // SSR-VSM-11: weld detection must stop the active discharge.
            if o.faults.contains(Fault::ContactorWeld) {
                prop_assert!(!o.active_discharge);
            }
            plant.step(&o, s);
            prev = o;
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2048, max_shrink_iters: 4000, .. ProptestConfig::default() })]

    #[test]
    fn vsm_safety_invariants(seq in prop::collection::vec(sample(), 5..40)) {
        let mut cov = Coverage::default();
        simulate(&seq, &mut cov)?;
    }
}

/// The invariants are only meaningful if the random walk actually reaches every
/// state and the important faults. This guards against a vacuous property test.
#[test]
fn random_walk_covers_all_states_and_key_faults() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, TestRunner};
    let mut runner = TestRunner::new(Config { cases: 1, ..Config::default() });
    let strat = prop::collection::vec(sample(), 5..40);
    let mut cov = Coverage::default();
    for _ in 0..1500 {
        let seq = strat.new_tree(&mut runner).expect("strategy").current();
        simulate(&seq, &mut cov).expect("invariants hold");
    }
    let names = ["Sleep", "Standby", "Precharge", "Ready", "Drive", "Charging", "SafeStop", "HvShutdown", "Fault"];
    for (k, n) in cov.states.iter().enumerate() {
        println!("state {:<10} visited {:>9} steps", names[k], n);
        assert!(*n > 0, "state {} never reached", names[k]);
    }
    for f in [Fault::Crash, Fault::HvilOpen, Fault::PrechargeTimeout, Fault::PrechargeTooFast, Fault::ContactorWeld,
              Fault::BmsCritical, Fault::MultiMotorLoss, Fault::IsolationFault, Fault::PedalConflict] {
        assert!(cov.faults & (1u64 << (f as u8)) != 0, "fault {} never exercised", f.name());
    }
}
