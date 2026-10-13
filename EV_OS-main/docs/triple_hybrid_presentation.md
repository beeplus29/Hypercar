# Hypercar Triple-Hybrid HV Topology & Development Phases

> **Engineering concept only:** Ratings and vehicle-performance figures below are provisional allocations or targets, not selected hardware ratings or validated vehicle results. See [the requirements baseline](triple_hybrid_spec.md).

---

## Slide 1: Triple-Hybrid HV Topology Overview
### The Ultimate Powertrain
- **Hydrogen Fuel Cell (HFC):** 150 kW net continuous output allocation at the DC link; 6.5 kg H2 inventory target. Stack ramp/efficiency maps are not yet integrated.
- **High-Energy Battery (HEB):** Current model is 200s3p, ~100 kWh nominal, 800 V class. It is not a selected hardware pack.
- **Supercapacitor Bank (SCB):** 2.0 kWh nominal; 500 kW for 5 s provisional bidirectional allocation. No SCB model/dispatch is implemented.
- **Traction motors:** Two rear Helix SPX242-175D and two front SPX242-94; datasheet motor-shaft totals are 1.8 MW peak (10 s) and 1.478 MW continuous under stated test conditions. The current twin still uses a generic motor model pending gearbox and inverter-channel details.
- **Vehicle targets:** >500-mile range and <2.0 s 0-60 mph remain targets. The twin does not validate range or a 0-60 mph run.

---

## Slide 2: Active Ground-Effect Downforce System (AGEDS)
### On-Demand 2000kg Vacuum
- **Hardware:** Twin Schübeler DS-82-DIA HST (120mm) Electric Ducted Fans.
- **Motors:** Integrated DSM6043-650 high-torque brushless motors (CW & CCW rotation to cancel torque).
- **Performance:** 28,000–30,500 RPM, producing up to 2000 kg of downforce via underbody skirt vacuum, even at a standstill.
- **Power Integration:** Draws up to ~18 kW combined; available HFC output is prioritized to offset the load and avoid draining the HEB.

---

## Slide 3: Why Triple-Hybrid?
### Addressing Hypercar Trade-offs
- **Thermal Management:** HFC offloads sustained current draw from the HEB, reducing pack temperatures during track sessions.
- **Battery Degradation:** SCB absorbs high-C regen spikes, extending the life of the HEB.
- **Weight vs Range:** 6.5 kg H2 represents about 216 kWh chemical LHV energy; delivered electricity depends on net fuel-cell efficiency, balance-of-plant load, and usable-fuel reserve. Range remains unvalidated.

---

## Slide 4: Power Allocation Strategy
### Smart Energy Management System (EMS)
- **Acceleration:** HFC supplies a slew-limited continuous baseline, SCB handles validated short bursts, and HEB supplies the remaining demand within live source limits.
- **Braking:** Allocate regen within live SCB/HEB charge limits; friction brakes cover the remainder. Twin regen acceptance limits still need explicit acceptance tests.
- **Cruising:** HFC operating point must come from the selected supplier's efficiency/ramp map. Preserve SCB headroom for both transients and regen rather than assuming a fixed 80% SOC target.

*These are target powertrain strategies; current Rust energy and twin code model the battery but do not yet implement HFC or SCB dispatch.*

---

## Slide 5: Recommended Development Phases
### Phase 1: Software & HIL Simulation
- Implement and test HFC-primary dispatch, battery buffering, SCB transient/regen limits, energy accounting, and source-fault handling.
- Hardware-in-the-Loop (HIL) simulation of the 800V DC link with simulated SCB, HEB, and HFC models.
- Validate control loop frequencies (SCB @ 1ms, HEB @ 10ms).

### Phase 2: Bench Testing (Mule 0)
- Integrate physical SCB and HEB with main inverters on a dyno.
- Emulate HFC via programmable DC power supply.
- Tune DC/DC converter PID controllers for seamless power blending.

### Phase 3: Full Integration (Mule 1)
- Install physical HFC and hydrogen tanks.
- Track testing: Validate thermal rejection targets and sustained performance mode over multiple hot laps.
- Refine state machine transition thresholds (e.g., Limp Home vs Performance Mode).
