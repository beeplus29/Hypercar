# HELIOS / EV_OS: DIY Hypercar Control Stack

![HELIOS](https://img.shields.io/badge/HELIOS-Hypercar-blue?style=for-the-badge)
![Language](https://img.shields.io/badge/Language-Rust-orange?style=for-the-badge)
![Status](https://img.shields.io/badge/Status-Active_Development-success?style=for-the-badge)

Welcome to the **HELIOS** project repository (also known as EV_OS). This repository contains the complete embedded Rust software stack, vehicle dynamics controllers, energy management systems, and hardware-in-the-loop (HIL) simulators for a state-of-the-art DIY Hypercar.

## 🏎️ Vehicle Concept & Powertrain

HELIOS is built around a revolutionary **Triple-Hybrid High-Voltage (HV) Topology**, paired with an **Active Ground-Effect Downforce System (AGEDS)**.

### Performance Targets
- **Max Range:** > 500 miles
- **0-60 mph:** < 2.0 seconds
- **Peak Power:** > 1500 kW (~2010 hp)
- **Downforce:** Up to 2000 kg on-demand vacuum (from a standstill)

The range and 0-60 mph performance targets remain unchanged with this powertrain configuration.

### Powertrain Architecture
1. **Hydrogen Fuel Cell (HFC):** The primary continuous power source, delivering 150 kW (6.5 kg H2 capacity) boosted to 800V to keep the system energized.
2. **High-Energy Battery (HEB):** 110 kWh, 800V pack for bulk energy storage, steady cruising, and moderate loads.
3. **Ultracapacitor Bank (SCB):** 2.0 kWh, 800V module for zero-lag 500 kW high-current bursts during hard acceleration and torque vectoring, and to absorb large regenerative-braking energy spikes.

These are vehicle design targets, not yet a complete software implementation. The current energy crate and digital twin model the battery, SOC, and thermal behavior; fuel-cell and ultracapacitor power allocation are planned work.

### Active Ground-Effect Downforce System (AGEDS)
Inspired by the McMurtry Spéirling, HELIOS features a skirted underbody evacuated by twin Schübeler DS-82-DIA HST (120mm) electric ducted fans. Drawing 18 kW combined, these fans create up to 2000 kg of downforce. 
- **Active Skirts:** Linear actuators drop flexible skirts to seal against the track in Performance Mode.
- **Debris Management:** A titanium FOD mesh protects the 30,000 RPM carbon blades from track debris and water ingestion.
- **Power:** The design intent is to prioritize available HFC output for this auxiliary load to preserve battery SoC; the current digital twin does not yet model the HFC.

## 🏗️ Software Architecture & Technologies (Software-Defined Vehicle)

HELIOS utilizes a state-of-the-art **Zonal Architecture** and the **Eclipse SDV (Software-Defined Vehicle)** stack over a high-speed Automotive Ethernet backbone, moving away from legacy CAN-heavy, monolithic ECU designs.

### Hardware Topology
- **Central High-Performance Computer (HPC):** An NVIDIA Thor / Jetson AGX Orin acts as the vehicle's brain. It runs QNX or Automotive Grade Linux and handles heavy-lifting computations: ADAS sensor fusion, complex Vehicle Dynamics Control (VDC) torque vectoring math, and the Energy Management System.
- **Zonal Controllers (NXP S32K3):** Distributed geographically (Front, Rear, Cabin). These act as ultra-reliable data routers. For example, the Rear Zonal controller subscribes to DDS commands from the Central HPC via Ethernet, translating them into localized CAN FD or PWM signals to physically drive the rear inverters and AGEDS downforce fans.
- **Low Voltage (LV) Management (Infinitybox INFIPM1Z):** A CAN-based intelligent power management system (Mastercell/Powercell NGX). The Zonal Controllers command the Infinitybox via CAN to handle 12V body loads (cooling pumps, lights, actuators), massively reducing LV wiring weight and replacing legacy relay/fuse boxes.

### Middleware & Eclipse SDV
- **Real-Time Data Backbone (Eclipse Cyclone DDS):** Certified to ISO 26262 ASIL D, Cyclone DDS handles massive data flows over Automotive Ethernet using a publish-subscribe model, eliminating CAN arbitration delays for critical torque commands.
- **Eclipse Ibeji:** Standardizes the digital twin architecture, managing the in-vehicle representation of hardware states (replacing legacy bespoke twin models).
- **Eclipse eCAL:** Used for scalable, high-throughput Inter-Process Communication (IPC) within the Central HPC.
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
- `crates/helios-zonal-fw`: Target firmware for the Zonal Controllers to route DDS to local actuators.

### Shared & Simulation
- `crates/helios-core`: Shared safety types (`#![no_std]`), vehicle modes, parameters, fault catalogues, and control math.
- `crates/helios-twin`: Digital Twin physics engine (Pacejka tyre models, load transfer, aero, thermal).
- `crates/helios-hil`: Hardware-in-the-Loop test runner and interfaces.

## 🛠️ Getting Started

### Prerequisites
- Ubuntu 24.04 (or similar Linux environment)
- Rust toolchain (`1.85+`)
- CMake `3.28+` and C/C++ build essentials
- Cyclone DDS `0.10.5`

### Building and Testing
The project heavily utilizes Cargo workspaces. To build and run tests natively in release mode:

```bash
# 1. Run all workspace unit and property tests
cargo test --workspace --release

# 2. Build the Digital Twin HIL simulator
cargo build -p helios-twin --release

# 3. Run all 12 Digital Twin Physics Scenarios
./target/release/helios-twin all --out out
```

### DDS Loopback Test
To verify the Cyclone DDS bindings and inter-process communication:
```bash
export CYCLONEDDS_URI="file://$PWD/config/dds/helios_loopback.xml"
cargo test -p helios-dds --release
```

## 📖 Documentation
Detailed engineering documentation and architecture specifications can be found in the [`docs/`](docs/) directory:
- [Triple-Hybrid Energy Parameter & Interface Spec](docs/triple_hybrid_spec.md)
- [Energy Management Logic](docs/energy_management_logic.md)
- [Initial Setup & Validation Report](docs/INITIAL_SETUP_REPORT.md)

---
*Developed by the HELIOS Vehicle Software Team.*