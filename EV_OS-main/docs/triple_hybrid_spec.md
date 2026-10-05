# Triple-Hybrid Energy Parameter and Interface Specification

## 1. Introduction
This document outlines the parameters and interfaces for the Triple-Hybrid Energy Storage System (THESS) intended for the Hypercar EV platform. The system integrates three distinct power sources to optimize energy density, power density, and thermal management.

## 2. System Architecture
The triple-hybrid system consists of:
1. **Hydrogen Fuel Cell (HFC):** Primary continuous power source and range extender, providing baseline power to the DC link.
2. **High-Energy Battery Pack (HEB):** 110 kWh bulk energy storage for cruising and sustained demand beyond the HFC output.
3. **High-Power Supercapacitor Bank (SCB):** Handles short high-frequency power bursts (rapid acceleration and torque vectoring) and high-power regenerative-braking spikes.

This is the target vehicle architecture. The current Rust energy model and digital twin implement the HEB model only; HFC and SCB models and their power-allocation logic remain future work.

### 2.1 Active Ground-Effect Downforce System (AGEDS)
- **Fans:** Twin Schübeler DS-82-DIA HST (120mm) Electric Ducted Fans (1x CW, 1x CCW to cancel torque).
- **Motors:** Integrated DSM6043-650 high-torque brushless motors.
- **Function:** Evacuates the underbody skirted cavity to produce up to 2000 kg of downforce from a standstill, similar to the McMurtry Spéirling.
- **Skirt Actuation:** Active linear actuators raise the flexible side and rear skirts during low-speed driving for ground clearance, and drop them flush with the track during Performance Mode to create the vacuum seal.
- **Debris Management:** A titanium mesh Foreign Object Debris (FOD) screen sits ahead of the fan intakes to pulverize/deflect track debris (rocks, rubber) before it can enter the 30,000 RPM carbon-fiber fan blades. A water-separator baffle manages rain/spray.

## 3. Parameter Specification (Initial Estimates)

| Parameter | High-Energy Battery (HEB) | Supercapacitor Bank (SCB) | Hydrogen Fuel Cell (HFC) | System Total |
| :--- | :--- | :--- | :--- | :--- |
| **Nominal Voltage (V)** | 800 V | 800 V | 400 V (boosted to 800V) | 800 V (DC Link) |
| **Energy Capacity (kWh)** | 110.0 kWh | 2.0 kWh | 6.5 kg H2 (~104 kWh eq.) | 216.0 kWh eq. (Est. >500 mi range) |
| **Peak Power Output (kW)** | 850 kW (10s) | 500 kW (5s) | 150 kW (continuous) | > 1500 kW (~2010 hp combined)|
| **Continuous Power (kW)**| 400 kW | 100 kW | 150 kW | 650 kW |
| **Charge Rate (Peak)** | 400 kW | 500 kW (regen) | N/A (H2 Refuel < 5 min) | N/A |
| **Operating Temp (°C)** | 15°C to 45°C | -40°C to 65°C | 60°C to 80°C | N/A |
| **Weight (kg)** | 550 kg | 90 kg | 135 kg (inc. tanks) | ~775 kg |

*Note: Values marked as initial estimates will be refined following hardware-in-the-loop (HIL) testing.*
*The listed peak source ratings sum to 1,500 kW; the separate >1,500 kW vehicle peak-power target is not demonstrated by these estimates and requires validated or increased source ratings.*

## 4. Hardware Interfaces

### 4.1 DC/DC Converters & Inverters
- **HEB to DC Link:** Direct connection via main contactors (pre-charge circuit included).
- **SCB to DC Link:** Bi-directional DC/DC converter (400kW peak) for precise voltage matching and rapid power injection/absorption.
- **HFC to DC Link:** Uni-directional DC/DC boost converter (150kW continuous) to step up 400V fuel cell output to the 800V main DC bus.
- **AGEDS Fan Controllers:** Twin high-voltage ESCs (Electronic Speed Controllers) drawing up to ~18 kW combined (9 kW per fan @ ~30,000 RPM) directly from the DC link or an intermediate LV bus.

### 4.2 Communication Interfaces
- **CAN-FD / Automotive Ethernet:** All energy sources communicate with the Vehicle Control Unit (VCU) via dual-redundant CAN-FD (500 kbps / 2 Mbps) or Automotive Ethernet (100BASE-T1).
- **Update Rate:**
  - SCB: 1 ms (high priority for transient handling)
  - HEB: 10 ms (standard priority)
  - HFC: 50 ms (lower priority, slower dynamics)

## 5. Software Interfaces (EV_OS)

### 5.1 Power Request API
```rust
struct PowerRequest {
    pub target_power_kw: f32,
    pub duration_ms: u32,
    pub is_regen: bool,
}

trait EnergySource {
    fn request_power(&mut self, req: PowerRequest) -> Result<f32, String>;
    fn get_available_power(&self) -> f32;
    fn get_soc(&self) -> f32;
    fn get_temperature(&self) -> f32;
}
```

### 5.2 Source-Specific Endpoints
- `/energy/heb/status`: Publishes HEB voltage, current, SoC, SoH, and cell temperatures.
- `/energy/scb/status`: Publishes SCB voltage, current, and module temperatures.
- `/energy/hfc/status`: Publishes HFC stack voltage, hydrogen flow rate, pressure, and coolant temperature.
