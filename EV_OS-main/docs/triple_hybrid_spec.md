# HELIOS Triple-Hybrid Powertrain Requirements Baseline

## 1. Status and scope

This document defines a **provisional system-engineering baseline** for the intended fuel-cell-primary powertrain. It is for architecture, simulation, and supplier sizing only. No source, converter, pack, or vehicle rating below is approved for hardware operation: supplier data, pack design, cooling design, electrical protection, and independent safety review are still required.

The fuel cell is the primary continuous source in normal hybrid operation. The battery is the bulk electrical store and supplies demand the fuel cell cannot follow. The ultracapacitor handles brief power transients and regenerative-braking peaks within its limits. The existing Rust energy model and digital twin implement the battery path only; they do not yet model or dispatch the fuel cell or ultracapacitor.

Vehicle range, acceleration, peak vehicle power, and downforce figures remain **vehicle targets**, not demonstrated requirements or acceptance results. In particular, the twin's 0-100 km/h launch scenario is not a 0-60 mph test, and there is no range-validation scenario.

## 2. Reconciled modeled baseline

The values below are read from `crates/helios-core/src/params.rs` and the battery state-of-power implementation. They describe the current software model, not selected or validated hardware.

| Modeled item | Current model value | Meaning / limitation |
|---|---:|---|
| Battery topology | 200s3p, 45 Ah per cell | 600 representative NMC cells |
| Battery nominal energy | ~99.9 kWh | 200 x 3 x 45 Ah x 3.7 V; replaces the unsupported 110 kWh figure |
| Pack nominal / min / max voltage | ~740 / 560 / 840 V | Based on 3.7 / 2.8 / 4.2 V per cell; “800 V class,” not fixed 800 V |
| Battery SOC test start | 90% | Used by twin launch/thermal scenarios; no validated usable-energy window |
| Discharge SOP horizon | 10 s | Voltage, modeled resistance, SOC, temperature, and 16C current limit are evaluated over this horizon |
| Charge current model limit | 8C | Not a validated pack charge rating; no twin scenario currently asserts a charge-power acceptance threshold |
| Cell thermal derating | Starts at 50°C; modeled maximum 60°C | Charging is additionally derated below 15°C and reaches zero at 0°C |
| Traction motor model | 4 x 630 kW peak | VDC applies a 1.5 MW system mechanical-power cap for the quad baseline |
| Twin launch battery-side peak | 1.544 MW observed; scenario ceiling 1.65 MW | DC-side input includes conversion losses. The 1.65 MW assertion is a broad model guardrail, not a hardware rating |

The existing scenario's **1.65 MW DC-side battery-only guardrail** is above its observed 1.544 MW battery-side launch peak. The quad twin separately caps traction at **1.5 MW mechanical**. These are distinct measurement points; neither value is a hardware rating or physical validation.

## 2.1 Motor set from supplied Helix datasheet

The selected motor configuration is **two rear SPX242-175D motors and two front SPX242-94 motors**, based on the supplied January 2026 Helix datasheet (`docs/SPX242-202601-Data-Sheet_EN_Helix (2).pdf`).

| Position / variant | Quantity | Peak power (10 s) | Continuous power | Peak / continuous torque | Maximum speed | Dry motor mass |
|---|---:|---:|---:|---:|---:|---:|
| Front SPX242-94 | 2 | 400 kW each | 278 kW each | 470 / 331 Nm each | 8,700 rpm | 34 kg each |
| Rear SPX242-175D | 2 | 500 kW each | 461 kW each | 973 / 548 Nm each | 17,750 rpm | 54 kg each |
| **Total motor set** | **4** | **1.8 MW for 10 s** | **1.478 MW continuous** | **2,886 Nm peak at motor shafts (arithmetic sum only)** | Per-variant | **176 kg dry motors** |

Helix specifies peak values at 800 V DC and 50°C coolant inlet for 10 seconds, and continuous values at 800 V DC with 15 L/min coolant flow and 50°C inlet. Its datasheet lists a 925 V maximum motor DC voltage, liquid cooling, 0–80°C coolant inlet, and 0.5–3.0 bar gauge inlet pressure. Treat these as datasheet limits/conditions, not a completed system qualification. The source notes ratings are typical operating/test-validated values and actual performance must be evaluated in the final system.

The 175D is a two-three-phase motor configuration. Confirm the number of physical CTI-4 inverters or inverter channels per unit with Helix; if each 175D uses two inverter units and each 94 one, the arrangement requires six inverter channels. Confirm AEM VCU300 support for that exact architecture. Motor torque and speed above are at the motor shafts: final drive ratios, wheel-side torque, inverter speed/phase-current limits, and efficiencies remain unresolved.

The current `helios-core`/`helios-twin` model does **not** yet represent this configuration. It still applies one generic 630 kW / 4,300 Nm / 2,800 rpm motor at each wheel and a 1.5 MW mechanical system cap. The 1.8 MW datasheet sum is not a vehicle output requirement and must not simply replace that cap: battery/fuel-cell/capacitor power, inverter limits, motor derating, gearing, and traction all constrain delivered vehicle power. Update the simulation only after gear ratios and inverter/channel topology are specified.

At the provisional HFC (150 kW), HEB sustained allocation (300 kW), and SCB continuous allocation (100 kW), the named source allocations sum to 550 kW DC before converter losses. This does not support operating the motor set at its 1.478 MW aggregate continuous rating. Even the provisional 1.65 MW DC-link burst depends on all three sources delivering their peak allocations concurrently and is not yet supported by the selected battery, capacitor bank, converters, or a source-sharing twin. Motor continuous/peak ratings are component capability ceilings, not system-level power promises.

## 3. Provisional component requirements

These are conservative architecture allocations for the next model iteration. “Provisional” means they must be checked against supplier maps and thermal/electrical design before hardware procurement or operation. No unverified value is a safety limit.

| Source | Energy / fuel basis | Continuous discharge allocation | Peak discharge allocation | Charge / regen allocation | Operating envelope and control |
|---|---|---:|---:|---:|---|
| Hydrogen fuel-cell system (HFC) | 6.5 kg H2 stored target. At 33.3 kWh/kg LHV, this is ~216 kWh chemical; at an assumed 45–55% net system efficiency it corresponds to ~97–119 kWh DC before usable-fuel reserve. | 150 kW net at the DC link, continuous only while the selected system's validated coolant, air, hydrogen, and stack limits permit. | No overload above 150 kW is allocated. Fuel-cell transient response is slew-limited by the selected stack/converter; obtain and encode the supplier ramp map rather than assuming instantaneous response. | No electrical charging allocation. | Primary continuous source in normal hybrid operation. No numerical stack temperature, hydrogen pressure, or converter input-voltage limit is assigned until hardware is selected; enforce OEM limits and independent leak/isolation, pressure, temperature, voltage/current, and converter fault monitoring. Fuel-cell output is never required to satisfy a fast torque transient. |
| High-energy battery (HEB) | 99.9 kWh nominal in the current model. A 200s13p candidate pack using the linked SA08 listing would be ~97.7 kWh nominal; 10–90% planning window only. | 300 kW is a provisional minimum system allocation for sustained balancing, not validated for a candidate cell pack. | Provisional hybrid peak allocation: 1.0 MW DC for up to 5 s only when HFC supplies 150 kW and SCB supplies 500 kW concurrently, yielding a 1.65 MW total DC-link burst. Beyond 5 s, or if either source is unavailable, torque/power must derate to remaining validated source capability. This envelope needs a hybrid twin scenario and supplier confirmation. | 150 kW / 10 s provisional battery regen cap is subject to the selected pack's live BMS SOP and cell qualification; not currently enforced by the twin. | Current model uses representative 45 Ah cells at 16C discharge, 8C charge, 2.8–4.2 V, and thermal derating from 50°C to zero at 60°C. SA08 listing instead gives 11.05 Ah, 5C continuous discharge, 8C pulse up to 30 s, 3C maximum charge, and 2.5–4.2 V. The model must be recalibrated before representing an SA08 pack. |
| Ultracapacitor bank (SCB) | 2.0 kWh nominal target. Plan a 20–80% working window (~1.2 kWh) to preserve charge and discharge headroom; verify actual usable energy over voltage and temperature. | 100 kW provisional bidirectional continuous allocation, subject to module and converter thermal ratings. | Up to 500 kW for 5 s. One ideal 500 kW / 5 s pulse transfers ~0.69 kWh, before losses; repeated pulses require an explicit recharge and thermal budget. | Up to 500 kW regen for 5 s, subject to the same energy, temperature, voltage, and converter constraints. | No numerical module voltage or temperature limits are assigned until a module is selected. Enforce supplier voltage, current, temperature, energy, and converter limits. No SCB model or dispatch exists in the current twin. |

The proposed source ratings are not additive vehicle ratings. The provisional 1.65 MW DC-link peak for up to 5 s assumes 150 kW HFC + 1.0 MW HEB + 500 kW SCB are simultaneously available. The present quad twin caps mechanical traction power at 1.5 MW and has a separate 1.65 MW DC battery-only launch guardrail; it does not model this hybrid source split. Source sharing, bus/converter limits, losses, and the post-5 s derate must be implemented before claiming a combined peak.

The 150 kW HFC allocation is **net DC output**, not stack gross output. A 6.5 kg hydrogen inventory is not 104 kWh of delivered electricity: ~216 kWh is the approximate chemical LHV energy, while delivered energy depends on net efficiency, usable-fuel reserve, and balance-of-plant consumption.

### 3.1 Candidate component compatibility gate

The linked SA08 listing would require approximately 200s13p (2,600 cells) for ~97.7 kWh nominal at 3.4 V/cell. At the listing's per-cell limits, this is 702 A continuous and 1,123 A for up to 30 s. At 680 V nominal, the ideal power products are approximately 477 kW continuous and 764 kW pulse before voltage sag, temperature, BMS, wiring, or converter derating. This does **not** support the provisional 1.0 MW HEB contribution to the hybrid peak, much less the twin's battery-only 1.544 MW launch result. Do not increase the cell current limits in software to bridge this gap.

Amprius's current [product catalog](https://amprius.com/products/) classifies SA08, SA19, SA30, and SA88 as High Power cells. The public category labels do not give the detailed limits needed to select a different SKU. A theoretical 200s23p SA08 pack (4,600 cells, ~172.8 kWh nominal, ~490 kg of cells before pack structure/cooling) would reach about 1.11 MW at 560 V only at the listed 8C pulse ceiling. This has little margin, is not pack-qualified, and already nearly consumes the former 550 kg pack mass target. It is a screening calculation, not a recommendation or selection.

At 200s, the SA08 listing implies ~680 V nominal, 840 V at 4.20 V/cell, and 500 V at 2.50 V/cell. This is an 800 V-class maximum bus, not 800 V nominal. A charger advertised for 550–850 V would not cover the full cell voltage range unless the pack cutoff and charging/wake behavior are designed accordingly.

The listed Maxwell BCAP3000 P300 K05 is a capacitor cell, not an HV module. As a rough bank illustration only, 300 cells in series at 2.7 V/cell would be 810 V maximum nominal cell rating; three parallel strings would contain 900 cells and about 2.7 kWh gross ideal energy. This does not establish safe operating voltage, useful energy window, module current/thermal capability, or 500 kW pulse capability. Obtain the current manufacturer datasheet and supplier engineering approval before retaining the SCB allocation.

The candidate fuel-cell link is ambiguous between a 10 kW M240 stack and a 20 kW listing title. Neither confirms a 150 kW net system. Confirm the exact stack, continuous net system output, and balance-of-plant ratings; if this low-power stack is intended for development, treat it as a test source rather than the vehicle's sole primary range-extender source.

Until the cell/pack, stack, capacitor bank, motor, inverter, and converter variants are confirmed as a compatible set, these figures are **screening calculations**, not final frozen hardware requirements. See [the candidate-parts compatibility assessment](hardware_manifest.md#candidate-parts-supplied-for-evaluation).

## 4. Power limits and fault responses

All source requests must be clamped to the lowest applicable limit from the source BMS/controller, converter, DC bus, motor/inverter, thermal model, and vehicle state. A missing, stale, invalid, or contradictory limit is treated as unavailable power, not as permission to use a nominal rating.

| Condition | Required supervisory response |
|---|---|
| HFC unavailable, hydrogen limit reached, or HFC/converter fault | Set HFC request to zero and isolate per OEM sequence. Continue only on HEB/SCB within their live limits; reduce torque and auxiliaries as needed. Enter a controlled degraded/limp mode when the remaining power or energy reserve is insufficient. |
| HEB over/under-voltage, over-current, over-temperature, isolation/BMS critical fault, or stale BMS data | Inhibit propulsion or apply the VSM-defined safe shutdown, open contactors only through the approved sequence, and latch/reset according to the validated safety concept. Never substitute a nominal battery rating for invalid BMS limits. |
| HEB low SOC or thermal derate | Reduce requested traction and nonessential loads; use available HFC output as the continuous source. Keep propulsion within the BMS state-of-power output. |
| SCB over/under-voltage, over-current, over-temperature, converter fault, or invalid telemetry | Command SCB power to zero and isolate it per converter procedure. HEB may take over only within its own live discharge/charge limit; otherwise reduce propulsion. |
| Regen exceeds SCB and HEB charge acceptance, either source is full/cold/hot/faulted, or telemetry is invalid | Limit electrical regen to the sum of live charge limits and blend the remainder to friction braking. Friction braking remains capable of the required vehicle deceleration without electrical regen. |
| DC-link or converter limit reached | Limit source requests and torque to the validated bus/converter envelope; on a protection trip, follow the VSM safe-stop/shutdown behavior. |
| Hydrogen leak, abnormal pressure, fire/crash signal, or HFC enclosure fault | Apply the selected tank/stack OEM emergency isolation and vehicle crash response. Do not rely on software power derating as the sole mitigation. |

The exact thresholds, fault latching, isolation sequence, and limp-home power are hardware- and safety-concept-dependent. The current EMS document is strategy-level only; these responses are requirements for implementation and verification, not claims that they are already implemented.

## 5. Interface and validation requirements

- HFC status and limits: net DC voltage/current/power, stack and coolant temperatures, hydrogen pressure/flow, available power, fault state, and timestamp/validity.
- HEB status and limits: pack voltage/current, cell voltage and temperature extrema, SOC/SOH, charge/discharge SOP with horizon, contactor/isolation state, fault state, and timestamp/validity.
- SCB status and limits: module voltage/current, SOC or energy estimate, temperature extrema, charge/discharge power limits, converter state, fault state, and timestamp/validity.
- Every power request carries a direction, requested power, duration/horizon, and operating mode. Every accepted response reports the clamped power and limiting reason.
- Regenerative braking is constrained by live source charge acceptance; friction-brake blending covers the remainder.
- Add energy-balance checks (HFC + HEB + SCB + regen, converter losses, auxiliaries, and traction), per-source limit tests, invalid/stale telemetry tests, and injected source/converter fault scenarios before claiming EMS simulation completion.
- Update twin acceptance criteria to assert the defined DC-side HEB cap separately from mechanical traction power, and add SCB/HFC limit assertions when their models are implemented.

## 6. Vehicle performance targets (not yet verified)

- Range: **>500 miles target**; no validated consumption cycle, drag/rolling-resistance calibration, usable-energy budget, or fuel-cell efficiency map currently supports it.
- 0–60 mph: **<2.0 s target**; current twin launch acceptance is 0–100 km/h in <=2.2 s for quad and <=3.2 s for dual layouts. These are simulation checks with model parameters, not physical validation or a 0–60 mph result.
- Peak vehicle power: **>1.5 MW target**; the quad twin uses a 1.5 MW mechanical traction cap and does not model the HFC or SCB. No >1.5 MW delivered vehicle power is demonstrated.

## 7. Hardware integration boundaries

- HEB connects to the DC link through the validated pack disconnect/precharge and protection architecture.
- HFC connects through a converter sized for its OEM-defined stack voltage/current envelope and 150 kW net provisional continuous allocation.
- SCB connects through a bidirectional converter sized for the 500 kW / 5 s provisional pulse envelope and sustained thermal operation.
- HV components, insulation coordination, contactors, fusing, current sensing, cooling, crash response, and hydrogen systems require supplier documentation and qualified engineering review. This document is not a wiring, commissioning, or HV operating procedure.

Use the [hardware-bench readiness checklist](bench_readiness_checklist.md) to
track evidence, unresolved decisions, and staged validation gates. It does not
authorize energization or vehicle testing.
