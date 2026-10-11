# HELIOS Software-Defined Vehicle (SDV) Architecture

## 1. Zonal Architecture Overview

The HELIOS hypercar abandons the traditional, monolithic ECU structure (with a single massive Vehicle Control Unit) in favor of a **Zonal Architecture** connected via a high-speed Automotive Ethernet backbone. 

### 1.1 Central High-Performance Computer (HPC)
**Hardware:** NVIDIA Thor or Jetson AGX Orin
**OS:** QNX Neutrino or Automotive Grade Linux (AGL)
**Role:** Intended central compute role; hardware and OS selection are not yet frozen.
**Planned software components:**
- `helios-vdc`: Vehicle Dynamics Control (Torque vectoring, Yaw-rate, aero strategy).
- `helios-energy`: Battery ECM, SOC estimation, and battery/motor thermal models. Fuel-cell and supercapacitor dispatch remain planned work.
- `helios-vhal-server`: The AAOS infotainment bridge.

The digital-twin simulation runs VSM/VDC steps at 1 kHz. There is not yet an HPC application that acquires vehicle sensors or publishes actuator torque commands.

### 1.2 Vehicle Control Runtime and Zonal Controllers
**Selected HPC/control platform family:** NXP S32G GoldBox/GreenBox; exact board and SoC derivative are not specified.
**Zonal MCU candidate:** 3x NXP S32K3 (Lockstep ARM Cortex-M7).
**RTOS:** Zephyr integration has started in `firmware/zephyr-control`, initially on `native_sim`. The checked-out Zephyr source does not provide a GoldBox/GreenBox S32G target, BSP, or peripheral drivers.
**Role:** The Zephyr app calls existing Rust `#![no_std]` VSM and VDC code through a narrow C ABI. It is a safe-output scaffold only: it deliberately rejects placeholder sensor data and is not connected to sensors, CAN, contactors, or inverters.
**Planned software components:**
- `helios-vsm`: Vehicle State Machine (HV contactors, HVIL).
- `helios-vdc`: vehicle dynamics control and torque allocation.
- `helios-can`: localized CAN FD networks for legacy sensors.
- `helios-energy`: no_std energy algorithms are available, but no validated target BMS data interface is defined; the host energy model is not yet called by the Zephyr app.
- Board-specific firmware, target BSP, sensor/actuator acquisition, CAN transport, and hardware safety interfaces remain unimplemented.

The Zephyr control facade applies 5 ms heartbeat-stale and 9 ms heartbeat-lost thresholds in its VSM configuration; due to the VSM's strict greater-than comparison, a 10 ms heartbeat age triggers the lost-heartbeat reaction. Its task-watchdog default is 5 ms (configurable up to 9 ms), with a 10 ms hardware-watchdog fallback when the board exposes a watchdog device. These are software integration settings, not validated fault-tolerant-time intervals or an ASIL claim. Hardware watchdog/reset behavior and independent de-energize-to-safe actuator paths must be validated on the exact board.

### 1.3 Low Voltage (LV) Power Management
**Hardware:** Infinitybox INFIPM1Z (Intelligent Power Management Kit)
**Role:** Body Control & 12V Load Distribution
**Integration:**
The INFIPM1Z replaces traditional relay and fuse boxes with a CAN-bus distributed architecture (Mastercell NGX and Powercell NGX modules). Instead of the Zonal Controllers manually toggling 12V relays for body functions (headlights, active aero actuators, cooling pumps), the Zonal Controllers translate DDS commands from the Central HPC into CAN messages. These CAN messages are sent to the Infinitybox Mastercell, which intelligently commands the distributed Powercells to drive the 12V loads. This further decentralizes the architecture and reduces LV wiring harness weight.

---

## 2. Eclipse SDV Stack Integration

HELIOS leverages the **Eclipse Foundation Software-Defined Vehicle** ecosystem to avoid rewriting base communication logic.

### 2.1 Eclipse Cyclone DDS
- **Purpose:** Publish/subscribe communication between processes and eligible networked components.
- **Implemented:** The `helios-dds` Rust crate wraps Cyclone DDS and generates typed support from `idl/helios/Telemetry.idl`. Local loopback round-trip tests and publisher/subscriber smoke-test binaries are present. The `helios-twin-dds` binary adds a 100 Hz SIL telemetry publisher.
- **SIL data contract:** `helios-twin::TwinTelemetry` is a versioned, transport-independent snapshot used by the DDS and eCAL SIL publishers. The DDS mapping converts radians to the existing telemetry IDL's road-wheel degrees.
- **Not yet deployed:** The checked-in Cyclone configuration is loopback-only. No validated vehicle Ethernet configuration, target sysroot/runtime image, zonal endpoint, or end-to-end latency/deadline evidence is provided here. DDS is not inherently deterministic or “instant,” does not remove the need for network scheduling and fault handling, and this project integration is not ISO 26262 certified. It must not be treated as the sole path for safety-critical actuation without a completed safety case and validated system design.

### 2.2 Eclipse Ibeji
- **Purpose:** Standardized Digital Twin architecture.
- **Implemented for hardware-free SIL:** `helios-ibeji-sil` implements the public Ibeji gRPC registration/provider/consumer wire interfaces for a loopback-only local registry. `helios-twin-ibeji` registers one versioned telemetry entity, subscribes a local consumer, and publishes simulated state using the same `TwinTelemetry` contract as DDS/eCAL.
- **Not a full Eclipse Ibeji deployment:** The local registry is a test harness, not the upstream Ibeji service. Chariott discovery, DTDL model validation, other Ibeji modules, TLS/authentication, persistence, and connection to an upstream Ibeji instance are not implemented. `Set` and `Invoke` are explicitly rejected; no simulated provider operation actuates a vehicle.

### 2.3 Eclipse eCAL (enhanced Communication Abstraction Layer)
- **Purpose:** Scalable Inter-Process Communication (IPC).
- **Implemented for SIL:** An optional `helios-twin-ecal` telemetry-only publisher uses the maintained `rustecal` binding and JSON messages on the `helios/sil/twin_telemetry` topic. It is separate from the Cyclone DDS publisher and has no actuator path.
- **Not yet deployed:** Building requires the native Eclipse eCAL 6+ runtime. The current Rust binding's build support covers Linux and macOS, not QNX. No production topic contract, Linux HPC image, IPC resource limits, or control-deadline isolation has been validated. Do not use eCAL in a control-critical path based on this SIL sample.

## 3. Deployment integration sequence

1. **SIL (implemented baseline):** Use the digital twin and loopback Cyclone DDS. The `helios-twin-dds` target publishes simulation telemetry only; it has no hardware actuator interface. Host scheduling is best-effort, not a real-time guarantee. The optional `helios-twin-ecal` target publishes the same versioned `TwinTelemetry` snapshot as JSON and requires a native eCAL 6+ installation. The `helios-twin-ibeji` target starts a local Ibeji-compatible registry/provider/consumer simulation with no hardware or external service required:

   ```sh
   # Cyclone DDS SIL
   CYCLONEDDS_URI="file://$PWD/config/dds/helios_loopback.xml" \
     cargo run -p helios-twin --features sil-dds --bin helios-twin-dds -- \
       --duration-s 10 --throttle 0.2

   # eCAL SIL (requires native eCAL 6+; set ECAL_HOME if installed non-system-wide)
   ECAL_HOME=/path/to/ecal \
     cargo run -p helios-twin --features sil-ecal --bin helios-twin-ecal -- \
       --duration-s 10 --throttle 0.2

   # Local hardware-free Ibeji-compatible flow
   cargo run -p helios-twin --features sil-ibeji --bin helios-twin-ibeji -- \
     --duration-s 10 --throttle 0.2
   ```

   Add automated schema, freshness, loss, and timing tests before using it as a deployment baseline.
2. **Central HPC:** After hardware and OS versions are fixed, build Cyclone DDS and eCAL against a pinned target sysroot, add a versioned Ethernet configuration, and verify CPU/memory limits, discovery, QoS, packet loss, and end-to-end timing. Add Ibeji as a non-control digital-twin interface only after selecting a concrete Ibeji API/model contract.
3. **Zephyr vehicle control:** The `firmware/zephyr-control` app links `helios-core`, `helios-vsm`, and `helios-vdc` on `native_sim`, with a 1 ms task period and 5 ms watchdog. It deliberately consumes invalid placeholder sensor data and has no vehicle I/O. The exact S32G board/BSP, target ABI, board drivers, CAN message validation, freshness checks, authority limits, independent safety path, and timing evidence are still required before any target build can command hardware. Do not treat this scaffold as a firmware release or safety case.

Target compile checks (compile-only, not target execution or hardware validation):

```sh
# Generic ARM64 Linux host logic; DDS/eCAL features intentionally excluded
cargo check --target aarch64-unknown-linux-gnu -p helios-twin

# Cortex-M7 shared logic only; this does not produce a firmware image
cargo check --target thumbv7em-none-eabihf \
  -p helios-core -p helios-vsm -p helios-can

# Zephyr native_sim application (requires the repository's Zephyr checkout and SDK)
west build -p always -b native_sim firmware/zephyr-control
```

These steps are integration work, not a functional-safety certification or authorization to connect the software to a vehicle.

The Ibeji SIL schemas and coverage boundaries are documented in
[`crates/helios-ibeji-sil/README.md`](../crates/helios-ibeji-sil/README.md).
The next Ibeji integration step is to select an upstream Ibeji release and DTDL
model and test this provider against that actual service, rather than the local
registry harness.
