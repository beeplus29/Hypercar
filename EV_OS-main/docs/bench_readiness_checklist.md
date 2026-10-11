# HELIOS Hardware-Bench Readiness Checklist

## Purpose and status

This checklist turns the [master specification blueprint](HELIOS_Master_Specification_Blueprint.md.pdf)
and [triple-hybrid requirements baseline](triple_hybrid_spec.md) into decision gates
for a first hardware bench. It is a planning and evidence-tracking document, not a
build instruction, wiring diagram, commissioning procedure, or authorization to
energize high voltage or hydrogen equipment.

**Current gate: NOT READY for an energized traction-power bench.**

The repository contains simulation and software-integration prototypes, not a
validated vehicle control system. The fuel-cell and ultracapacitor dispatch paths
are not implemented in the twin; the battery model is not qualified to a selected
pack; and the motor/inverter, source, and safety architecture is not frozen.
Range, acceleration, and peak vehicle power remain targets. A qualified HV and
functional-safety engineering review is required before hardware selection,
construction, or operation.

Use these checklist states:

- `[ ]` Open / no acceptable evidence recorded
- `[x]` Complete / evidence linked and reviewed
- `[!]` Blocked / decision or evidence is missing
- `N/A` Not applicable, with rationale recorded

Do not mark an item complete based on a vendor headline, a simulation pass, or an
unreviewed assumption. Record evidence, reviewer, and date in the evidence log.

## Gate 0 — Define the bench scope

- [ ] **Bench purpose and boundary documented.** Specify which functions are in
  scope (for example, controller I/O, low-voltage state-machine tests, or a
  separately reviewed power-conversion test) and explicitly exclude anything not
  covered by the safety review.
- [ ] **Test configuration identified.** Record exact hardware variants, firmware
  and calibration versions, interface revisions, and whether the setup is
  hardware-free SIL, controller-on-bench, or power hardware.
- [ ] **Hazard analysis and test authorization completed.** A qualified reviewer
  has approved the hazards, safeguards, personnel roles, emergency response,
  facility suitability, and permitted operating envelope for this specific
  configuration.
- [ ] **Stop conditions and emergency responsibilities documented.** Identify
  who can stop a test, the independent means to reach a safe state, and how an
  incident or unexpected result is handled.

**Gate 0 exit:** The test scope, hazards, personnel, facility, and authorization
are documented and approved. This is not authorization for later configurations.

## Gate 1 — Freeze the engineering inputs

### Powertrain and energy sources

- [ ] **Motor and mechanical topology confirmed.** Confirm the two rear
  SPX242-175D and two front SPX242-94 motor variants, their operating envelopes,
  cooling requirements, final-drive ratios, gearing, differential arrangement,
  and wheel-speed limits from supplier documentation.
- [ ] **Inverter/channel mapping confirmed in writing.** Resolve the 175D
  dual-three-phase topology, CTI-4 inverter quantity/channel arrangement, motor
  feedback and resolver interfaces, derating, peak-power duration, continuous
  limits, and the AEM VCU300 compatibility and supported calibration path.
- [ ] **Battery pack limits supplier-backed.** Select the actual cells and pack
  integrator; obtain pack-level voltage, current-versus-time, usable energy,
  temperature, cooling, charge acceptance, BMS state-of-power, and fault limits.
  Reconcile these with the modeled 200s3p, ~99.9 kWh, 16C/8C assumptions before
  using them in tests.
- [ ] **Fuel-cell system rating and interface confirmed.** Resolve the supplied
  M240 listing's 10/20 kW ambiguity and whether it is a development stack or
  part of a multi-stack system. Obtain net DC output, converter, transient/ramp,
  balance-of-plant, operating, shutdown, and fault data. Do not treat the
  provisional 150 kW net allocation as an available rating.
- [ ] **Ultracapacitor bank and converter defined.** Select a complete bank and
  bidirectional converter; obtain voltage, current, power-duration, usable
  energy, balancing, thermal, and fault limits. A BCAP3000 cell is not an HV
  bank. Do not treat 2 kWh, 100 kW continuous, or 500 kW/5 s as verified ratings.
- [ ] **Common DC-link envelope reconciled.** Check pack, source-converter,
  inverter, auxiliaries, cabling, disconnects, contactors, fuses, current
  sensing, insulation monitoring, and precharge/discharge design as one system.
  Verify voltage/current/time limits and protection coordination with qualified
  engineering review.
- [ ] **Power-time profile agreed.** Define system continuous demand, peak demand
  and duration, recovery/repetition interval, regen acceptance, auxiliary loads,
  and required derating when any source is unavailable. Reconcile the profile
  against the twin's battery-only launch observation and motor-shaft ratings;
  do not add independent component maxima and call the sum a system rating.
- [ ] **Hydrogen scope explicitly decided.** If hydrogen hardware is excluded
  from the first bench, record that exclusion and how the interface will be
  represented safely. If included, require a separate supplier-backed system
  design, facility review, and approved hydrogen-specific hazard controls before
  the configuration can be reconsidered.

### Control and interfaces

- [ ] **Control ownership and authority defined.** Identify which selected
  controller owns torque requests, inverter enables, source power commands,
  HV sequencing, and independent shutdown. Do not assume DDS, eCAL, HPC, Android,
  or an unvalidated Teensy path is an independent safety mechanism.
- [ ] **Interface contracts versioned.** Document signal units, ranges, validity,
  timestamps/freshness, sequence handling, startup behavior, timeout behavior,
  fault ownership, and permitted command authority for each connected device.
- [ ] **Loss-of-communication behavior verified by requirements.** Set watchdog
  and stale-data thresholds from the safety analysis and hardware timing budget.
  A proposed 10 ms timeout is not accepted by default; prove that the complete
  detection-to-safe-state response meets the approved requirement.
- [ ] **No unsupported safety claim remains.** Treat ASIL classifications,
  certification, DDS safety, and safety goals in the blueprint as unverified
  until supported by an applicable safety case, supplier evidence, and system
  assessment.

**Gate 1 exit:** The bench bill of materials, source limits, interface contracts,
power-time envelope, and safety requirements have reviewed evidence and no
unresolved incompatibility relevant to the defined scope.

## Gate 2 — Verify software and non-energized integration

- [ ] **Requirements-to-test traceability established.** Each in-scope requirement
  maps to implementation, test case, expected result, and evidence record.
- [ ] **SIL tests pass for the pinned revision.** Run targeted Rust unit and
  scenario tests with recorded toolchain, features, and revision. Clearly label
  SIL results; they are not HIL or hardware validation.
- [ ] **Fault and boundary tests cover invalid inputs.** Test missing, stale,
  out-of-range, contradictory, and lost source/controller data; startup and
  shutdown transitions; power-limit clamping; regen acceptance; and fault
  latching/reset behavior as applicable to the implemented scope.
- [ ] **Energy and power accounting checked.** For any implemented source-sharing
  path, assert per-source limits, bus limits, conversion losses, auxiliaries,
  traction demand, and charge/regen balance. Do not claim source-sharing
  validation while the HFC/SCB paths are absent.
- [ ] **Controller I/O tested without traction power.** Confirm signal mapping,
  polarity, scaling, safe defaults, command rejection, watchdog observation,
  and independent disable behavior using an approved non-energized setup.
- [ ] **Build and test dependencies recorded.** The most recent workspace-wide
  test results are recorded with native dependencies, target platform, exact
  command, and exclusions. A passing subset is not a full workspace pass.

**Gate 2 exit:** In-scope software tests and non-energized I/O checks pass on the
pinned configuration, and failures/limitations are recorded.

## Gate 3 — Independent bench design and readiness review

- [ ] **Electrical design reviewed.** Qualified reviewers approve the schematic,
  protection coordination, insulation/grounding strategy, enclosure and
  interlock design, measurement points, and defined limits for the exact bench.
- [ ] **Protection and shutdown paths reviewed.** Confirm the independent
  protection functions and safe response to communications loss, controller
  reset, sensor disagreement, source/converter fault, over/under-voltage,
  over-current, over-temperature, isolation fault, and emergency stop, where
  applicable. Thresholds and response times come from approved requirements
  and supplier data, not this checklist.
- [ ] **Instrumentation and evidence plan approved.** Identify calibrated
  instruments, measurement uncertainty, sampling/time synchronization needs,
  data retention, test procedures, acceptance criteria, and responsible reviewer.
- [ ] **Cooling and environmental limits established.** Confirm coolant,
  flow/pressure, inlet temperature, ambient range, thermal sensing, and
  measurement/abort criteria for every included component using its exact
  datasheet and reviewed system design.
- [ ] **Configuration and change control in place.** Record wiring/harness,
  firmware, calibration, parameter, and test-procedure versions. A material
  change reopens the affected gate.
- [ ] **Reviewers explicitly authorize the next test stage.** Record named
  approvals and scope. An approval for a low-voltage or emulator test does not
  authorize an HV or hydrogen test.

**Gate 3 exit:** The exact test configuration has documented independent
engineering approval. Only the scope specifically approved may proceed.

## Gate 4 — Execute staged validation

Define stage-specific acceptance criteria before testing. Progress only after
reviewing results and recording authorization for the next stage.

1. `[ ]` **Hardware-free SIL:** Verify modeled limits, scenarios, and fault
   behavior. No hardware claims.
2. `[ ]` **Controller and I/O bench without traction power:** Verify interfaces,
   state transitions, fault injection, watchdogs, logging, and safe outputs.
3. `[ ]` **Power-interface/emulator tests, if separately approved:** Verify
   interface and protection behavior only within the approved emulator and
   facility envelope.
4. `[ ]` **Energized component or DC-link tests, if separately approved:** Use
   only the reviewed configuration and pre-approved procedure; capture
   electrical, thermal, timing, and fault evidence.
5. `[ ]` **Integrated source/inverter/dynamometer tests, if separately approved:**
   Use supplier-backed limits and a separately reviewed test plan. Do not infer
   vehicle readiness from component or dyno results.
6. `[ ]` **Vehicle installation and closed-course testing:** Requires a new
   vehicle-level design, hazard, regulatory, and test-readiness review. This
   checklist does not authorize road testing.

## Current decisions and blockers

| ID | Decision / blocker | Current evidence and impact | Owner | State |
|---|---|---|---|---|
| D-01 | Select/freeze battery pack and operating envelope | Current twin battery assumptions are not supplier-qualified; SA08 seller data does not substantiate the modeled battery-only launch power or provisional 1.0 MW hybrid contribution. | TBD | Open |
| D-02 | Confirm fuel-cell system | Supplied M240 listing is ambiguous between 10 and 20 kW; neither establishes the provisional 150 kW net allocation. | TBD | Open |
| D-03 | Define ultracapacitor bank and converter | BCAP3000 is a cell; the proposed bank, converter, useful energy, and pulse limits are not selected or validated. | TBD | Open |
| D-04 | Freeze motor/inverter and mechanical topology | Confirm the 175D phase-system mapping, CTI-4 quantity/channels, VCU300 support, final drive, and gearing. Twin remains a generic motor model. | TBD | Open |
| D-05 | Approve safety concept and test scope | Safety claims and target timeouts in the blueprint are not a completed safety case or independently reviewed bench authorization. | TBD | Open |
| D-06 | Establish repeatable validation baseline | Complete workspace tests now pass on this macOS host, including DDS loopback. Native dependency setup was performed locally and is not yet captured in a reusable bootstrap/CI configuration. | TBD | Open (reproducibility) |

## Latest software validation record

**Date:** 2026-10-08  
**Source state:** Uncommitted working tree; results are for the tested checkout and
are not a release or hardware-validation record.

- Local test prerequisites: Cyclone DDS **0.10.5** built from upstream tag
  `2cdd114cbd18340c606573b4cc8dc20cc161ec5a`; local CMake 4.4.4; vendored
  `protoc` 3.3.0; and a macOS DDS loopback profile using `lo0`.
- `cargo test --workspace --quiet` — **full workspace passed, 75 tests passed,
  0 failed**. The run included `helios-voice`, `helios-obd`,
  `helios-vhal-server`, and Cyclone DDS tests.
- `cargo run --quiet -p helios-twin --bin helios-twin -- all --out /tmp/helios-twin-readiness`
  — **12 scenarios passed, 0 failed**.
- Native dependency setup was local and manual: Cyclone DDS was built in the
  ignored `.deps/` directory, CMake was installed under `.deps/python-tools`,
  and `PROTOC` pointed to the cached macOS-aarch64 vendored compiler. The
  commands are not yet automated for a clean checkout.

The scenario results validate only the current simulation model and its stated
acceptance checks. They do not validate the SPX242 motor set, a selected battery
pack, HFC/SCB dispatch, an inverter, a controller interface, an energized bench,
or a physical vehicle. No safety or hardware qualification is implied.

## Evidence and decision log

For each completed checklist item or closed decision, record:

| Date | Checklist item / decision ID | Evidence link or artifact and revision | Result / rationale | Reviewer(s) | Follow-up |
|---|---|---|---|---|---|
| 2026-10-08 | Gate 2 software baseline / D-06 | Current uncommitted macOS checkout; commands and versions recorded above | Full workspace: 75 tests passed; all 12 twin scenarios passed. CPAL/VAD resources are now owned on dedicated threads, and macOS DDS loopback tests pass. | TBD | Automate and pin native dependency bootstrap, then rerun from a clean checkout/CI environment. |

Keep vendor documents, calculations, approved schematics, test procedures, raw
logs, analysis, and sign-offs revision-controlled or in an explicitly identified
engineering record system. Do not store credentials or other secrets in this
repository.

## Related project references

- [Triple-hybrid powertrain requirements](triple_hybrid_spec.md)
- [Candidate hardware manifest](hardware_manifest.md)
- [Zonal SDV architecture and deployment stages](architecture_zonal_sdv.md)
- [Master specification blueprint](HELIOS_Master_Specification_Blueprint.md.pdf)
