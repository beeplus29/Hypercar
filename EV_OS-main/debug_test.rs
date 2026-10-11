use helios_vdc::*;
use helios_core::params::HELIOS_BASELINE;
use helios_core::modes::DriveMode;

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

fn main() {
    let mut vdc = Vdc::new(HELIOS_BASELINE, 0.001);
    let mut i = rolling(60.0);
    i.accel_pedal = 1.0;
    for step in 0..200 {
        let o = vdc.step(&i);
        if step == 199 {
            println!("fx_achieved: {}", o.fx_achieved);
            println!("fx_demand: {}", o.fx_demand);
            println!("tc_active: {:?}", o.tc_active);
            println!("abs_active: {:?}", o.abs_active);
            println!("slip: {:?}", o.slip);
            println!("motor_torque: {:?}", o.motor_torque);
            println!("p_elec_est: {}", o.p_elec_est);
        }
    }
}
