# Hypercar Triple-Hybrid HV Topology & Development Phases

---

## Slide 1: Triple-Hybrid HV Topology Overview
### The Ultimate Powertrain
- **Hydrogen Fuel Cell (HFC):** 400V (boosted to 800V DC Link). 150kW continuous primary baseline power (6.5 kg H2 capacity) for sustained operation and range extension.
- **High-Energy Battery (HEB):** 800V, 110kWh bulk energy storage for cruising and sustained power beyond the HFC output; the >500-mile range remains a vehicle target.
- **Supercapacitor Bank (SCB):** 800V, 2.0kWh. Zero-lag transient response for hard acceleration and 500kW regen braking; the <2 second 0-60 mph target remains unchanged.

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
- **Weight vs Range:** 6.5kg of H2 provides ~104kWh equivalent energy at a fraction of the weight of adding 104kWh of battery cells.

---

## Slide 4: Power Allocation Strategy
### Smart Energy Management System (EMS)
- **Acceleration:** HFC supplies its continuous baseline, SCB handles short high-current bursts, and HEB supplies sustained demand and balances the remaining load.
- **Braking:** SCB handles primary 500kW regen spike. Excess flows to HEB (max 150kW).
- **Cruising:** HFC runs at optimal efficiency curve (~50kW) to maintain HEB state of charge. SCB held at 80% for instant overtake power.

*These are target powertrain strategies; current Rust energy and twin code model the battery but do not yet implement HFC or SCB dispatch.*

---

## Slide 5: Recommended Development Phases
### Phase 1: Software & HIL Simulation
- Finalize EV_OS energy management state machine logic.
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
