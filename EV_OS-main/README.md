# HELIOS / EV_OS: DIY Hypercar Control Stack

![HELIOS](https://img.shields.io/badge/HELIOS-Hypercar-blue?style=for-the-badge)
![Language](https://img.shields.io/badge/Language-Rust-orange?style=for-the-badge)
![Status](https://img.shields.io/badge/Status-Active_Development-success?style=for-the-badge)

Welcome to the **HELIOS** project repository (also known as EV_OS). This repository contains Rust control and vehicle-dynamics prototypes, battery and digital-twin models, and early Android Automotive UI work for a DIY hypercar concept. It is not a complete or safety-qualified embedded vehicle stack, and its current HIL crate and simulation results do not constitute hardware validation.

## 🏎️ Vehicle Concept & Powertrain

HELIOS is built around a revolutionary **Triple-Hybrid High-Voltage (HV) Topology**, paired with an **Active Ground-Effect Downforce System (AGEDS)**.

### Vehicle Performance Targets (Unvalidated)
- **Range:** > 500 miles target
- **0-60 mph:** < 2.0 seconds target
- **Peak Power:** > 1500 kW target
- **Downforce:** Up to 2000 kg target

These figures are concept targets, not measured vehicle results. The digital twin currently checks 0-100 km/h launch behavior for selected layouts; it does not validate 0-60 mph or range.

### Powertrain Architecture
1. **Hydrogen Fuel Cell (HFC):** Primary continuous-source target of 150 kW net at the DC link, with 6.5 kg hydrogen inventory target. Stack/converter dynamics and dispatch are not modeled.
2. **High-Energy Battery (HEB):** The current model describes a 200s3p pack at about 100 kWh nominal and an 800 V-class voltage range. This is a model baseline, not a selected pack rating.
3. **Ultracapacitor Bank (SCB):** 2.0 kWh nominal and 500 kW for 5 s are provisional component-allocation targets; no SCB model or dispatch is implemented.
4. **Traction Motors:** Planned configuration is two rear Helix SPX242-175D and two front SPX242-94 units. The supplied datasheet rates the set at 1.8 MW peak for 10 s and 1.478 MW continuous at motor shafts under stated 800 V/cooling conditions. These ratings are not yet represented in the twin; transmission ratios and inverter-channel integration remain open.

See [the powertrain requirements baseline](docs/triple_hybrid_spec.md) for modeled limits, provisional component allocations, fault responses, and unvalidated performance targets. The [hardware-bench readiness checklist](docs/bench_readiness_checklist.md) tracks engineering evidence and staged test gates; it is not authorization to energize hardware. Fuel-cell-primary dispatch is a design requirement, not current runtime behavior.

### Active Ground-Effect Downforce System (AGEDS)
Inspired by the McMurtry Spéirling, HELIOS features a skirted underbody evacuated by twin Schübeler DS-82-DIA HST (120mm) electric ducted fans. Drawing 18 kW combined, these fans create up to 2000 kg of downforce. 
- **Active Skirts:** Linear actuators drop flexible skirts to seal against the track in Performance Mode.
- **Debris Management:** A titanium FOD mesh protects the 30,000 RPM carbon blades from track debris and water ingestion.
- **Power:** The design intent is to prioritize available HFC output for this auxiliary load to preserve battery SoC; the current digital twin does not yet model the HFC.

## 🏗️ Software Architecture & Technologies (Software-Defined Vehicle)

HELIOS is prototyping a **Zonal Architecture** and selected **Eclipse SDV (Software-Defined Vehicle)** components. The hardware, OS, network configuration, and deployment interfaces are not yet frozen; the architecture below distinguishes prototypes from planned vehicle integration.

### Hardware Topology
- **Central High-Performance Computer (HPC):** NVIDIA Thor / Jetson AGX Orin with QNX or Automotive Grade Linux are candidate platforms, not validated deployment targets.
- **Zonal Controllers (NXP S32K3):** Proposed front/rear/cabin controllers. A validated DDS-to-CAN/PWM actuator route is not yet implemented.
- **Low Voltage (LV) Management (Infinitybox INFIPM1Z):** A CAN-based intelligent power management system (Mastercell/Powercell NGX). The Zonal Controllers command the Infinitybox via CAN to handle 12V body loads (cooling pumps, lights, actuators), massively reducing LV wiring weight and replacing legacy relay/fuse boxes.

### Middleware & Eclipse SDV
- **Eclipse Cyclone DDS:** Integrated through the `helios-dds` Rust wrapper, generated HELIOS IDL types, and SIL publisher/subscriber tools. The checked-in configuration is loopback-only; vehicle Ethernet deployment, deadlines, and target runtime have not been validated. DDS does not inherently guarantee deterministic delivery, and this project integration is not ISO 26262 certified.
- **Eclipse Ibeji:** The `helios-ibeji-sil` crate and `helios-twin-ibeji` runner exercise the public Ibeji gRPC registration/subscribe/publish interfaces against a local registry harness. This is hardware-free SIL, not the full upstream Ibeji runtime; see [the adapter limits](crates/helios-ibeji-sil/README.md).
- **Eclipse eCAL:** An optional SIL-only twin telemetry publisher is available via `helios-twin`'s `sil-ecal` feature. It requires the native eCAL 6+ SDK; the current Rust binding supports Linux/macOS, not QNX. HPC IPC and vehicle deployment are not validated.
- **Infotainment Bridge:** Integrates with AOSP Android 17 using a native Rust gRPC Vehicle Hardware Abstraction Layer (VHAL) server.

## 📂 Repository Structure

The project is structured as a Cargo workspace containing the following core crates and directories, organized by deployment target:

### Central HPC (NVIDIA Thor / AGX Orin)
- `crates/helios-vdc`: Vehicle Dynamics Control (torque vectoring, yaw-rate tracking, TC/ABS, aero strategy).
- `crates/helios-energy`: Battery ECM, SOC EKF, and battery/motor thermal models. Fuel-cell and ultracapacitor power allocation are not yet implemented.
- `crates/helios-vhal-server`: Android Automotive OS (AAOS) VHAL gRPC server implementation.
- `crates/helios-dds` / `helios-dds-sys`: Safe Rust bindings and FFI for Cyclone DDS communication.

### Zonal Controllers (NXP S32K3 / Cortex-M7)
- `crates/helios-vsm`: Vehicle State Machine (HV sequencing, contactor control, HVIL reactions).
- `crates/helios-can`: CAN FD table-driven codec, E2E protection, and matrix management.
- `crates/helios-core`, `helios-vsm`, `helios-vdc`, `helios-can`: reusable `no_std` logic.
- `firmware/zephyr-control`: initial Zephyr application scaffold, currently configured for `native_sim`.
- `crates/helios-zephyr-control-ffi`: static Rust control library exposing VSM/VDC over a narrow C ABI.
- Zephyr's control facade uses 5/9 ms stale/lost heartbeat thresholds (10 ms triggers the strict-greater-than VSM fault), a 5 ms task watchdog (maximum 9 ms), and a 10 ms hardware-watchdog fallback. It is not connected to sensors or actuators. The selected S32G GoldBox/GreenBox has no exact board/BSP integration in this repository.

### Shared & Simulation
- `crates/helios-core`: Shared safety types (`#![no_std]`), vehicle modes, parameters, fault catalogues, and control math.
- `crates/helios-twin`: Digital Twin physics engine (Pacejka tyre models, load transfer, aero, thermal).
- `crates/helios-ibeji-sil`: Loopback-only Ibeji-compatible protocol harness for hardware-free SIL.
- `crates/helios-hil`: Hardware-in-the-Loop test runner and interfaces.
- `crates/helios-twin::TwinTelemetry`: Versioned transport-neutral SIL snapshot shared by optional DDS/eCAL publishers.

## 🛠️ Getting Started

### Prerequisites
- Ubuntu 24.04 (or similar Linux environment)
- Rust toolchain (`1.85+`)
- CMake `3.28+` and C/C++ build essentials
- Cyclone DDS `0.10.5`

### Building and Testing
The project heavily utilizes Cargo workspaces. On macOS or Linux, bootstrap the
pinned native Cyclone DDS dependency into the ignored `.deps/` directory. The
script uses an installed CMake; on macOS only, it installs pinned CMake 4.4.4
locally if CMake is missing. Linux hosts need CMake 3.28+ and a C/C++ toolchain.
It fetches Cyclone DDS 0.10.5 from the pinned upstream commit and does not
install Cyclone DDS system-wide.

```bash
# 1. Prepare local native dependencies (requires Git and network access)
tools/bootstrap-native.sh

# 2. Run all workspace unit and property tests
tools/cargo-local.sh test --workspace --release

# 3. Build and run all Digital Twin Physics Scenarios
tools/cargo-local.sh build -p helios-twin --release
./target/release/helios-twin all --out out

# Optional: faster optimized iteration (no LTO; final binary is not as optimized)
tools/cargo-local.sh build -p helios-twin --profile release-fast
```

The standard `release` profile keeps full optimization and thin LTO, with debug
information removed and linker symbols stripped to reduce deployment size.
`release-fast` disables LTO and uses parallel code generation to shorten
optimized build times. Use the standard `release` profile for runtime
performance comparisons and deployment candidates; neither profile is a
hardware-safety qualification.

### Zephyr Control Scaffold

With the existing Zephyr checkout and SDK configured, build and run the host
simulator integration:

```bash
west build -d build/zephyr-control -p always -b native_sim firmware/zephyr-control
west build -d build/zephyr-control -t run
```

Run this on Linux: Zephyr's `native_sim` POSIX architecture does not support
macOS. This validates only Zephyr startup, Rust linking, and task/watchdog integration.
The app rejects placeholder sensor input and does not send CAN or actuator
commands. The S32G GoldBox/GreenBox hardware build remains blocked on the exact
board, BSP, Rust target ABI, and verified I/O/safety contract. See
[the Zephyr migration status](firmware/zephyr-control/README.md).

### Android release packaging

The `cluster` and `infotainment` application modules enable R8 minification and
resource shrinking for release APKs. Build them with:

```bash
cd android
./gradlew -Pkotlin.compiler.execution.strategy=in-process \
  -x lintVitalAnalyzeRelease \
  :cluster:assembleRelease :infotainment:assembleRelease
```

Gradle build caching and parallel project execution are enabled. The separate
legacy `android/app` prototype is not included in the current Gradle
`settings.gradle.kts`, so its APK must not be treated as validated by this
command. The lint-analysis task is excluded here because the checked-in
AGP 8.5.0 / compileSdk 35 combination fails in the local lint analyzer; run
release lint with a compatible Android Gradle Plugin/SDK toolchain before
shipping. The Kotlin compiler is run in-process because the local Kotlin
daemon crashes during compilation.

### DDS Loopback Test
To verify the Cyclone DDS bindings and inter-process communication:
```bash
CYCLONEDDS_URI="file://$PWD/config/dds/helios_loopback.xml" \
  tools/cargo-local.sh test -p helios-dds --release
```

The DDS loopback profile above targets Linux hosts. On macOS, the DDS tests
automatically use `config/dds/helios_loopback_macos.xml` with the `lo0`
interface. `tools/cargo-local.sh` exports the local shared-library path and
uses the existing vendored `protoc` build dependency for the VHAL server.

### Optional eCAL SIL Publisher
With the native Eclipse eCAL 6+ SDK installed (set `ECAL_HOME` when it is not
installed system-wide), publish simulated telemetry to the eCAL topic
`helios/sil/twin_telemetry`:
```bash
ECAL_HOME=/path/to/ecal cargo run -p helios-twin --features sil-ecal \
  --bin helios-twin-ecal -- --duration-s 10 --throttle 0.2
```
This is an IPC experiment only. It has no vehicle hardware interface and is not
a real-time or safety-qualified control path. See
[the SDV architecture and deployment stages](docs/architecture_zonal_sdv.md).

### Ibeji-compatible Hardware-free SIL
Run the full simulation → registry → provider → subscriber loop locally:
```bash
cargo run -p helios-twin --features sil-ibeji --bin helios-twin-ibeji -- \
  --duration-s 10 --throttle 0.2
```
It binds ephemeral loopback ports, registers a read-only telemetry entity, and
delivers versioned JSON snapshots to a simulated consumer. It does not run the
upstream Ibeji service and accepts no actuator commands.

### Developer Hardware Console
Run the opt-in, local-only hardware inventory and profile UI with guided
component templates, optional advanced connection profiles, inventory checks,
search, and JSON export:
```bash
cargo run -p helios-dev-console
```
Then open `http://127.0.0.1:4318`. The console stores inventory, uploaded
documents, and connection profiles under `~/.local/share/helios-dev-console/`
(override the inventory file path with `HELIOS_DEV_CONSOLE_DATA`). PDF, TXT,
and CSV files are read locally; PNG and JPEG images use Tesseract OCR, which
must be installed separately and available on `PATH`. Detected electrical and
mechanical values are suggestions with source excerpts, not verified ratings;
review them against the original document before adding them to a part. Uploads
are limited to 15 MB each. The console can report Linux SocketCAN interface
status, but only saves profiles; it does not send commands to vehicle hardware.
This developer tool is not included in Android application packages.

## 📖 Documentation
Detailed engineering documentation and architecture specifications can be found in the [`docs/`](docs/) directory:
- [Triple-Hybrid Energy Parameter & Interface Spec](docs/triple_hybrid_spec.md)
- [Energy Management Logic](docs/energy_management_logic.md)
- [Initial Setup & Validation Report](docs/INITIAL_SETUP_REPORT.md)

---
*Developed by the HELIOS Vehicle Software Team.*