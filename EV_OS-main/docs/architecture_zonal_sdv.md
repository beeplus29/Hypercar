# HELIOS Software-Defined Vehicle (SDV) Architecture

## 1. Zonal Architecture Overview

The HELIOS hypercar abandons the traditional, monolithic ECU structure (with a single massive Vehicle Control Unit) in favor of a **Zonal Architecture** connected via a high-speed Automotive Ethernet backbone. 

### 1.1 Central High-Performance Computer (HPC)
**Hardware:** NVIDIA Thor or Jetson AGX Orin
**OS:** QNX Neutrino or Automotive Grade Linux (AGL)
**Role:** The "Brain" of the vehicle.
**Deployed Crates:**
- `helios-vdc`: Vehicle Dynamics Control (Torque vectoring, Yaw-rate, aero strategy).
- `helios-energy`: Battery ECM, SOC estimation, and battery/motor thermal models. Fuel-cell and supercapacitor dispatch remain planned work.
- `helios-vhal-server`: The AAOS infotainment bridge.

The Central HPC computes the massive math required for sensor fusion and torque vectoring. It publishes resulting torque commands at 1000 Hz.

### 1.2 Zonal Controllers
**Hardware:** 3x NXP S32K3 (Lockstep ARM Cortex-M7)
**OS:** Bare-metal (`#![no_std]`) Rust
**Role:** The "Nerves" and "Muscles". Located at the Front, Rear, and Cabin.
**Deployed Crates:**
- `helios-vsm`: Vehicle State Machine (HV contactors, HVIL).
- `helios-can`: localized CAN FD networks for legacy sensors.
- `helios-zonal-fw`: Target firmware.

### 1.3 Low Voltage (LV) Power Management
**Hardware:** Infinitybox INFIPM1Z (Intelligent Power Management Kit)
**Role:** Body Control & 12V Load Distribution
**Integration:**
The INFIPM1Z replaces traditional relay and fuse boxes with a CAN-bus distributed architecture (Mastercell NGX and Powercell NGX modules). Instead of the Zonal Controllers manually toggling 12V relays for body functions (headlights, active aero actuators, cooling pumps), the Zonal Controllers translate DDS commands from the Central HPC into CAN messages. These CAN messages are sent to the Infinitybox Mastercell, which intelligently commands the distributed Powercells to drive the 12V loads. This further decentralizes the architecture and reduces LV wiring harness weight.

---

## 2. Eclipse SDV Stack Integration

HELIOS leverages the **Eclipse Foundation Software-Defined Vehicle** ecosystem to avoid rewriting base communication logic.

### 2.1 Eclipse Cyclone DDS
- **Purpose:** The real-time data backbone.
- **Implementation:** Wrapped by our `helios-dds` Rust crate. Used for all critical communication between the Central HPC and the Zonal Controllers over Ethernet. Certified to ISO 26262 ASIL D, it uses a Publish-Subscribe model that completely eliminates CAN arbitration delays.

### 2.2 Eclipse Ibeji
- **Purpose:** Standardized Digital Twin architecture.
- **Implementation:** Extends our `helios-twin` crate. Ibeji provides a standardized interface for representing the hardware states (like battery thermals and Pacejka tire states), making it easy to mirror the car's state in the cloud for predictive maintenance and track-day analytics.

### 2.3 Eclipse eCAL (enhanced Communication Abstraction Layer)
- **Purpose:** Scalable Inter-Process Communication (IPC).
- **Implementation:** Used *internally* on the Central HPC. While Cyclone DDS handles distributed network traffic to the Zonal Controllers, eCAL is used for zero-copy, high-throughput IPC between heavy processes on the NVIDIA Thor chip (e.g., passing raw camera/LIDAR arrays from Sensor Fusion to the `helios-vdc` Torque Vectoring process).
