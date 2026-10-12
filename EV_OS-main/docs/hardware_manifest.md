# HELIOS Conceptual Vehicle Compute & Control Hardware Manifest

This is a conceptual candidate list, not a selected bill of materials or an approved vehicle design. Verify all candidate hardware, ratings, interfaces, supply status, safety evidence, and compatibility with suppliers and qualified vehicle engineers before procurement or use. See the [powertrain requirements baseline](triple_hybrid_spec.md) for provisional source envelopes. No listed architecture or software has been safety-qualified for vehicle use.

## Candidate parts supplied for evaluation

The following are candidates, not confirmed selections. Different product alternatives in the same row are not assumed to be mutually compatible.

| Subsystem | Candidate(s) | Initial compatibility assessment |
|---|---|---|
| Vehicle control | [AEM VCU300, 30-8100](https://www.aemelectronics.com/products/ev_conversions/vehicle_control_unit/parts/30-8100) | AEM describes up to four motor/inverter systems and three CAN networks, which is directionally suitable for the proposed four-motor topology. Confirm the exact BMS, inverter, motor, charger, and contactor combinations against AEM's supported-applications list and calibration support before selection. It is not evidence by itself of a safety-qualified system. |
| Motors / mechanical drive | Two rear [Helix SPX242-175D](https://www.ehelix.com/products/spx242/) and two front [Helix SPX242-94](https://www.ehelix.com/products/spx242/); [Hewland powertrain listing](https://powertrain.hewland.com/browse-products/?singleproduct=620) is a candidate mechanical-drive component | Helix's supplied January 2026 datasheet rates each rear 175D at 500 kW peak for 10 s / 461 kW continuous, 973 / 548 Nm peak/continuous torque, and 17,750 rpm maximum. Each front 94 is rated 400 / 278 kW, 470 / 331 Nm, and 8,700 rpm maximum. Nominal totals are 1.8 MW peak for 10 s and 1.478 MW continuous at the datasheet's specified 800 V and cooling conditions. Motor-shaft ratings are not wheel torque/power. Exact Hewland ratio, final drive/differential arrangement, inverter channels (the 175D is dual 3-phase), and VCU support must be confirmed before finalizing the twin or harness. |
| Traction inverter | Helix CTI-4 SiC is the motor datasheet's listed inverter pairing; [Cascadia Motion CM-350SiC](https://www.cascadiamotion.com/cm-350sic) remains an alternative only | The January 2026 Helix CTI-4 datasheet specifies a 3-phase SiC inverter with 550 kW peak and 300 kW continuous, 750 Arms peak phase current, 435 Arms continuous phase current, 925 V max DC, 500 V minimum DC, 10.5 kg mass, 40 kHz max PWM, 10–20 L/min liquid cooling, and IP67/IP6K9K. The 175D has two independent 3-phase stator systems; confirm whether each rear motor uses two CTI-4 units/channels, which could mean up to six inverter channels total with two front SPX242-94s. Verify DC voltage over the full pack range, phase current/duration, resolver support, fault behavior, and VCU300 support. |
| Battery cell | [Amprius SA08, Tenergy listing](https://power.tenergy.com/amprius-sa08-3-4v-11-05ah-li-ion-polymer-rechargeable-pouch-cell-battery/) | The seller lists 3.40 V nominal, 11.05 Ah / 37.57 Wh typical, 4.20 V charge limit, 2.50 V end-of-discharge, 54 A (5C) max continuous discharge, 86.4 A (8C) pulse up to 30 s, and 32.4 A (3C) max charge. The seller says it is available only as a cell option in a Tenergy-designed pack and is not for individual sale. These data conflict with the twin's 45 Ah, 16C discharge, 8C charge and 2.8–4.2 V model cell. Obtain the cell datasheet and an integrator's written pack-level limits before adopting it. |
| Higher-power Amprius cell candidates | [Amprius product catalog](https://amprius.com/products/) lists SA08, SA19, SA30, and SA88 in its High Power category. | Public catalog classification alone does not provide the exact current-vs-voltage, pulse-duration, thermal, cycle-life, or pack-level limits needed to select any one of them for HELIOS. Request current cell datasheets and a pack integrator's qualified limits from Amprius. Do not assume a catalog "High Power" label means the cell meets the vehicle's required pulse profile. |
| Ultracapacitor cell | [Maxwell BCAP3000 P300 K05](https://www.digikey.com/en/products/detail/maxwell-technologies/BCAP3000-P300-K05/24366611) | This is a low-voltage cell, not an 800 V bank or a 500 kW converter. A series/parallel bank, balancing, containment, fusing, and a bidirectional DC/DC stage must be engineered. Confirm the latest manufacturer datasheet and cell current/thermal limits; 2.0 kWh and 500 kW are not established by selecting this part number alone. |
| Fuel-cell stack | [Hyfindr PEM metallic stack listing](https://hyfindr.com/en/shop/products/pem-fuel-cell-metallic-stack-20-kw) | The supplied link/listing is ambiguous about whether the offered M240 stack is 10 kW or 20 kW. Either is far below the provisional 150 kW net system allocation as a single unit. Confirm continuous net system output, stack operating envelope, start/stop and ramp behavior, balance-of-plant needs, and whether a multi-stack system is intended. Do not assume stack ratings add directly to net DC output. |
| Hydrogen storage | [700 bar Type IV cylinder listing](https://hyfindr.com/en/shop/products/hydrogen-type-4-cylinder-700-bar-143l-2) | Verify the exact cylinder variant and approved installed-system capacity. A cylinder's geometric volume is not its usable hydrogen mass; include regulator, shutoff valves, PRD/vent routing, crash protection, leak detection, and applicable certification in the tank-system design. |
| On-board charging / DC-DC | [Ovar 11 kW bidirectional OBC / 3 kW DC-DC listing](https://ovartech.com/product/bidirectional-11kw-obc-3kw-dc-dc-550-850vdc-for-lifepo4-battery/) | The advertised 550–850 V range may align with a 200-series SA08 pack only while pack voltage is at least 550 V; 200s at the cell's 2.50 V end-of-discharge is 500 V. Define a compatible pack cutoff/charge wake-up strategy and confirm whether the OBC and 3 kW DC-DC are actually included in one configured unit. This is not a traction or fuel-cell boost converter. |
| Charge inlet / communication | [NACS inlet candidate](https://amprevolt.com/products/nacs-charging-inlet-fast-charge-capable); [Sensata EVCC](https://www.sensata.com/products/electric-vehicle-communication-controllers/evcc) | Confirm the inlet's exact voltage/current ratings, approved use, cable/thermal limits, and compatibility with the selected EVCC, OBC, and DC fast-charge protocol. An inlet alone does not implement charging communication, contactor interlocks, or charge control. |
| HV isolation monitoring | [Bender IR155 series](https://www.benderinc.com/products/ground-fault-monitoring-ungrounded/isometer-ir155-03-04-series/) | This is an insulation-monitoring device (IMD), not a surge arrestor/suppressor. Select the correct variant for the bus voltage and system capacitance, then validate its interface and fault response. Surge/transient protection is a separate design function. |
| HV distribution / protection | [GIGAVAC GX110CEB](https://www.tti.com/content/ttiinc/en/apps/part-detail.html?partsNumber=GX110CEB&mfgShortname=GIG); [Mersen MEV100A100-4 100 A fuse](https://us.rs-online.com/product/mersen/mev100a100-4/73942483/); [Eaton 170M6528](https://www.eaton.com/my/en-us/skuPage.170M6528.html); candidate Exxelia busbars and TE connectors | Verify DC voltage, continuous and interrupt current, precharge/discharge duty, fault-current rating, I²t coordination, temperature rise, creepage/clearance, and connector/cable compatibility as one system. A 100 A fuse at 800 V corresponds to only 80 kW nominal and cannot be assumed to protect a megawatt-class traction bus; it may only be suitable for a separately sized branch. No fuse or contactor is approved by this list. |
| HV auxiliaries | [Guchen 800 V compressor](https://www.guchen-eac.com/what/high-voltage-electric-compressor/800v-electrically-driven-ac-compressor.html); [Sevcon Hi-V converter](https://www.thunderstruck-ev.com/sevcon-hv-converter.html); [EPC 18k converter](https://epicpower.es/product/epc-18k-890i/) | Select by exact part number, input/output voltage, power, isolation, cooling, EMC, and control interface. Count their loads in the battery/fuel-cell continuous power and thermal budgets. |

### Explicit sizing conflicts to resolve

1. **Battery power:** A 200s13p pack using the reseller's typical SA08 figures would contain 2,600 cells and about 97.7 kWh nominal at 3.4 V/cell. Its listed current ceilings are 702 A continuous and 1,123 A for up to 30 s. At 680 V nominal, the ideal power products are about 477 kW continuous and 764 kW pulse, before voltage sag, temperature, wiring, BMS, or converter derating. This is not compatible with the twin's 1.544 MW battery-only launch result. The cell's pulse rating is also only specified to 30 s, so it does not substantiate the model's 10 s pack SOP without pack qualification.
2. **SA08 candidate pack size:** If the HFC is delivering 150 kW and the SCB 500 kW simultaneously, a 1.65 MW total DC-link burst leaves about 1.0 MW for the HEB during the SCB's 5 s pulse. A theoretical 200s23p SA08 pack (4,600 cells, ~172.8 kWh nominal, ~490 kg of cells before pack structure/cooling) would reach about 1.11 MW at 560 V only at the listed 8C pulse ceiling. This leaves little margin and is not qualified; the bare-cell mass already approaches the former 550 kg battery-pack target. After the SCB pulse ends, sustaining the same total demand would require more HEB power, so the vehicle's required full-power duration and derating profile must be specified. This calculation does not make SA08 a suitable selected pack.
3. **Pack voltage:** 200s SA08 is about 680 V nominal, 840 V at 4.20 V/cell, and 500 V at the listed 2.50 V end-of-discharge. That is not an 800 V nominal pack. It can sit inside the Ovar listing's 550–850 V range only with a cell/pack cutoff that keeps the battery above 550 V; prove the resulting usable SOC and charge recovery behavior.
4. **Fuel-cell power:** The linked stack listing is only 10/20 kW-class per its conflicting listing/title information, not a confirmed 150 kW net system. Treat it as a possible development stack only until the supplier confirms the actual delivered continuous system rating.
5. **Ultracapacitor scale:** A BCAP3000 cell cannot connect directly to an 800 V bus. Establish a series/parallel bank and converter design from the manufacturer's voltage, current, balancing, and thermal data before retaining the provisional 2 kWh / 500 kW targets.
6. **Traction system:** Motor, inverter, battery, VCU, and gearing must be selected as a matched set. The chosen motor variants provide a datasheet-based shaft-power envelope, but the twin still uses a generic 4 x 630 kW, 4,300 Nm, 2,800 rpm per-corner actuator and does not model the 94/175D split, gear reduction, six-channel topology, or motor-specific continuous/thermal limits. Until exact Hewland ratios and inverter/control interfaces are confirmed, twin wheel torque, motor speed, DC voltage/current, and acceleration results are not hardware-derived.

**Do not change the 1.65 MW scenario guardrail into a hardware requirement for the SA08-based pack.** Either select a cell/pack capable of the required power with documented limits, or revise vehicle peak-power and acceleration targets and retune the digital twin to an achievable, supplier-backed envelope. Do not raise cell C-rate assumptions to make the modeled target pass.

**Amprius selection status:** SA08 is the only supplied Amprius cell with a public seller current rating found in this review, and that rating does not substantiate the modeled battery-only launch peak or the 550 kg pack target. Amprius lists other high-power cell models, but their detailed cell ratings are not publicly established here. The next valid selection step is to obtain those datasheets and require a pack-level proposal that satisfies the vehicle's defined DC power-vs-time profile, usable energy, maximum pack voltage, thermal envelope, and mass target. Until then, the Amprius model is **unselected**.

## 1. Central Compute, Networking, & Infotainment (3-Tier Safety Co-Processor Architecture)

### Tier 1: High-Compute Advisory & Perception (QM / Non-ASIL)
* **Central HPC (The AI Brain):**
  * *Hardware:* NVIDIA Jetson AGX Thor Developer Kit (or Jetson AGX Orin)
  * *Compliance Level:* **QM (Quality Management / Non-ASIL)**. Commercial developer kits lack hardware lockstep safety cores and deterministic fault-recovery mechanisms mandated by ISO 26262 for direct actuator authority.
  * *Sandboxed Role:* Exclusively advisory. Runs perception, LiDAR/camera sensor fusion, neural trajectory generation, and Android Automotive OS (AAOS) infotainment (`helios-vhal-server`). Emits proposed trajectories/torques (`HPC_AdvisoryCmd`) over Cyclone DDS / Automotive Ethernet to the ASIL firewall. It is **never** permitted to drive physical actuators directly.
  * *Physical Hardening Requirements for Vehicle Installation:*
    1. **Power Conditioning (ISO 7637-2 / ISO 16750-2):** Isolated DC-DC voltage conditioner protecting against cranking brownouts (~6V) and inductive load dumps (up to 40V–100V).
    2. **Thermal & Ingress Protection:** Enclosed in an IP67 CNC-machined aluminum chassis coupled to a dedicated liquid-cooling cold plate plumbed into the auxiliary low-temperature loop.
    3. **Shock & Vibration Isolation:** Chassis-mounted via silicone/wire-rope vibration isolators with ruggedized Amphenol/M12 locking connectors to prevent track vibration solder fatigue. (Alternatively, utilize a pre-built rugged edge mission computer from Neousys/Systel).

### Tier 2: Vehicle Gateway & Safety Supervisor (ISO 26262 ASIL D)
* **Central Gateway & Safety Firewall:**
  * *Hardware:* **NXP GoldBox-3 (S32G3)** (Lockstep Arm Cortex-M7 safety cores)
  * *Compliance Level:* **ISO 26262 ASIL D**
  * *Role:* Acts as the automotive safety firewall between the non-ASIL Thor and the physical actuators. It executes `helios-vdc::supervisor` and `helios-vsm`:
    1. **Brake-Throttle Override (BTO):** Vetoes all advisory propulsion to 0 Nm if the driver touches the brake pedal.
    2. **Slew-Rate Limiting:** Enforces $\le 50$ Nm/ms torque delta limits to prevent driveline shock.
    3. **Tire Adhesion Ceiling:** Clamps requested differential yaw moments to the friction circle ($M_z \le 0.85 \cdot \mu \cdot F_z \cdot \frac{\text{track}}{2}$).
    4. **Liveness & E2E Watchdog:** Monitors packet age ($< 50$ ms) and AUTOSAR E2E Profile 5 CRC16/counters; trips to `AdvisoryFirewallTrip` on failure.

### Connectivity & Cloud Telemetry
* **Cloud Telemetry & Connectivity:**
  * *Hardware:* **NXP OrangeBox**
  * *Role:* Wireless domain controller handling 5G cloud telemetry (for Eclipse Ibeji digital twins), Wi-Fi 6, and Ultra-Wideband (UWB) for smartphone digital keyless entry.

## 2. Powertrain & Execution Control (Tier 3)

### Tier 3: Hard Real-Time Execution Controller (Turnkey Motorsport ECU)
* **Powertrain Master Controller:**
  * *Hardware:* **AEM VCU 300 (Part 30-8100)**
  * *Why VCU 300 over NXP GreenBox 3:*
    * **Development Speed:** The VCU 300 features turnkey precharge sequencing, contactor state machines, and APPS dual-redundant pedal decoding configured via the graphical **AEMcal** suite. Replacing it with an NXP GreenBox 3 would require writing thousands of lines of bare-metal C/AUTOSAR drivers, memory-mapped registers, and low-level contactor weld-detection logic from scratch.
    * **Direct Inverter Integration:** Native CAN profiles for multi-inverter setups (Helix CTI-4 SiC channels) and high-voltage BMS communications.
    * **Role:** Receives validated, supervisor-sanitized torque requests via CAN FD and executes hard real-time motor phase currents, contactor interlocks, and thermal management.

*Path Alternatives & Evaluation:*
* **Path A (Custom Rust Zonal Scaffold):** NXP S32K3 zonal microcontrollers running `#![no_std]` `helios-vsm` and `firmware/zephyr-control`.
* **Path B (Turnkey Motorsport Architecture - Recommended):** AEM VCU 300 as the dedicated Master ECU, receiving sanitized torque targets from the NXP GoldBox supervisor firewall.
* **Path C (Hybrid Supervisory):** NXP GoldBox S32G3 running `helios-vdc::supervisor` for 1 kHz safety envelope enforcement, dispatching sanitized CAN FD frames down to the AEM VCU 300.

## 3. High-Voltage (HV) Powertrain Modules
* **Traction Inverters (quantity TBD):**
  * *Hardware:* Candidate Helix CTI-4 or Cascadia Motion CM-350SiC; select one family only after obtaining exact datasheets and VCU compatibility confirmation.
  * *Role:* Converts DC bus power to motor phase power. Final quantity depends on motor topology and selected inverter capability; four is not yet established.
* **Battery Management System (BMS) / iBDU:**
  * *Hardware:* Candidate BMS/iBDU and cell-monitor functions; component selection is open.
  * *Role:* Monitors cell voltage/temperature/current and controls pack disconnects according to the selected pack's validated limits. The current software model is approximately 100 kWh nominal and does not establish a solid-state cell design.
* **Supercapacitor DC-DC Converter:**
  * *Hardware:* Bidirectional converter sized against a provisional 500 kW / 5 s SCB allocation; vendor and topology selection open.
  * *Role:* Manages SCB charge/discharge only within validated module and converter limits. The 2.0 kWh SCB is a target, not a selected component.
* **Fuel Cell DC-DC Converter:**
  * *Hardware:* Not selected. The existing 150 kW single-stage boost entry is a target only; size it to the chosen stack's full operating-voltage/current envelope and the confirmed net system output.
* **AGEDS Fan Controllers (x2):**
  * *Hardware:* High-Voltage 200A+ Electronic Speed Controllers (ESCs).
  * *Role:* Drives the twin 30,000 RPM Schübeler fans for the active downforce system.
* **On-Board Charger (OBC):**
  * *Hardware:* Candidate Ovar bidirectional 11 kW OBC with advertised 550–850 V DC range and 3 kW DC-DC auxiliary converter.
  * *Role:* AC charging / auxiliary conversion subject to exact configured model, pack voltage window, communication, and supplier confirmation; it is not an external DC fast charger or traction DC-DC converter.

## 4. Low-Voltage (LV) & Body Control
* **Infinitybox Mastercell NGX:**
  * *Role:* The CAN-based brain of the 12V body control system. Listens to the Cabin Zonal controller to trigger body events.
* **Infinitybox Powercell NGX (Multiple):**
  * *Role:* Distributed smart relay modules that physically switch 12V power to cooling pumps, headlights, skirt linear actuators, and cabin fans.

## 5. Safety & Chassis Modules
* **Insulation Monitoring Device (IMD):**
  * *Hardware:* Bender IR155.
  * *Role:* Constantly checks for high-voltage leaks to the chassis, triggering a safe shutdown if resistance drops.
* **Electronic Power Assisted Steering (EPAS) Controller:**
  * *Hardware:* DCE dual-MGU steering controller.
  * *Role:* Steer-by-wire / assisted steering actuator module.
* **Brake-by-Wire / ABS Controller:**
  * *Hardware:* High-performance motorsport ABS unit (e.g., Bosch Motorsport M5).
  * *Role:* Handles physical hydraulic brake pressure to the AP Racing calipers if regenerative braking limits are exceeded.
