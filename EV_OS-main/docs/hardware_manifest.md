# HELIOS Final Vehicle Compute & Control Hardware Manifest

This document outlines all Electronic Control Units (ECUs), Vehicle Control Units (VCUs), and compute modules required to build the final HELIOS Triple-Hybrid hypercar using the Zonal Software-Defined Vehicle (SDV) architecture.

## 1. Central Compute, Networking, & Infotainment
* **Central HPC (The Brain):** 
  * *Hardware:* NVIDIA DRIVE Thor (or Jetson AGX Orin)
  * *Role:* Runs QNX/Linux for ADAS sensor fusion, high-level math, and the Android Automotive OS (AAOS) infotainment.
* **Central Vehicle Gateway:**
  * *Hardware:* **NXP GoldBox-3 (S32G3)**
  * *Role:* The central Ethernet and CAN router. It securely routes Cyclone DDS traffic between the NVIDIA Thor and the rest of the car, acting as the ultimate Service-Oriented Gateway.
* **Cloud Telemetry & Connectivity:**
  * *Hardware:* **NXP OrangeBox**
  * *Role:* Wireless domain controller handling 5G cloud telemetry (for Eclipse Ibeji digital twins), Wi-Fi 6, and Ultra-Wideband (UWB) for smartphone digital keyless entry.

## 2. Powertrain & Zonal Control (Two Paths)

*Path A: The 100% Custom Rust Route*
* **Powertrain Domain Controller / Zonal Routers:**
  * *Hardware:* **NXP GreenBox-3 (S32E)** and/or **NXP S32K3** microcontrollers.
  * *Role:* Runs custom `#![no_std]` Rust (`helios-vdc`, `helios-vsm`) for hard real-time ASIL-D torque vectoring, inverter control, and High-Voltage sequencing.

*Path B: The Proven Motorsport Route*
* **Powertrain Master Controller:**
  * *Hardware:* **AEM VCU300**
  * *Role:* Replaces the custom Rust vehicle state machine. The AEM VCU300 manages all 4 traction inverters, executes the High-Voltage pre-charge/shutdown sequences, and handles torque vectoring via its 3 independent CAN networks. It connects to the GoldBox-3 via CAN to report telemetry back to the Central HPC.

*Path C: The Ultimate Hybrid Supervisory Architecture (Using Both)*
* **Vehicle Dynamics Engine (Supervisory):**
  * *Hardware:* **NXP GreenBox-3 (S32E)** running custom Rust (`helios-vdc`).
  * *Role:* Calculates hyper-advanced, 1000Hz torque vectoring matrices, yaw-rate tracking, and Active Downforce (AGEDS) logic. It computes exactly how much torque each wheel needs to maximize grip, and sends these targets to the AEM VCU300.
* **High-Voltage Execution Controller:**
  * *Hardware:* **AEM VCU300**
  * *Role:* Acts as the bulletproof safety layer. It handles the terrifying High-Voltage pre-charge sequencing, contactor management, and inverter fault checking. It receives the dynamic torque targets from the GreenBox, validates them against safe battery limits, and safely commands the 4 traction inverters.

## 3. High-Voltage (HV) Powertrain Modules
* **Traction Inverters (x4):**
  * *Hardware:* Cascadia Motion CM-350SIC (or Donut Lab proprietary 800V SiC inverters)
  * *Role:* Converts DC bus power to 3-phase AC for the four independent 21" wheel motors.
* **Battery Management System (BMS) / iBDU:**
  * *Hardware:* HIITIO Intelligent Battery Disconnect Unit (iBDU) + Cell Monitor boards.
  * *Role:* Monitors cell voltage/temp of the 110kWh solid-state High-Energy Battery (HEB) and controls the GIGAVAC contactors.
* **Supercapacitor DC-DC Converter:**
  * *Hardware:* 500kW Bi-directional 800V DC-DC Converter.
  * *Role:* Manages the aggressive charge/discharge spikes of the 2.0kWh Supercapacitor Bank (SCB).
* **Fuel Cell DC-DC Boost Converter:**
  * *Hardware:* 150kW Uni-directional Boost Converter.
  * *Role:* Steps up the 400V Fuel Cell output to the 800V main DC Link.
* **AGEDS Fan Controllers (x2):**
  * *Hardware:* High-Voltage 200A+ Electronic Speed Controllers (ESCs).
  * *Role:* Drives the twin 30,000 RPM Schübeler fans for the active downforce system.
* **On-Board Charger (OBC):**
  * *Hardware:* Ovar 550-850VDC OBC.
  * *Role:* Handles AC grid charging when plugged in at the paddock.

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
