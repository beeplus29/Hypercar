# HELIOS Zephyr vehicle-control application

This app is the first Zephyr runtime integration point. It calls the existing
`no_std` Rust `helios-core`, `helios-vsm`, and `helios-vdc` crates through a
small C ABI facade. The app is an integration scaffold: sensor acquisition,
CAN drivers, actuator outputs, and an exact NXP target are not connected or
safety validated.

## Target support boundary

The chosen hardware family is NXP S32G GoldBox/GreenBox, but the repository has
no exact board part number, SoC derivative, BSP, pinout, safety partitioning, or
vendor support package. The checked-out upstream Zephyr tree does not provide a
GoldBox/GreenBox S32G board target. This project does not invent that support.
`native_sim` runs only on Linux and validates the build/link/thread structure;
it does not emulate target timing, peripherals, lockstep safety, or S32G
behavior.

## Build on the existing Zephyr checkout

The repository has a local Zephyr source checkout under `zephyr/`, which is
kept separate from this application. `native_sim` requires Linux, west, and a
Zephyr toolchain:

```sh
west build -d build/zephyr-control -p always -b native_sim firmware/zephyr-control
west build -d build/zephyr-control -t run
```

CMake detects the host Rust target for `native_sim`. A hardware build requires
an explicit `-DRUST_TARGET=<triple>` that matches the chosen Zephyr CPU and
ABI. Do not select the existing Cortex-M7 target for an S32G application core
without confirming the actual control core and board toolchain.

## Control and watchdog boundary

The Zephyr control thread is configured for a 1 ms period with priority 1. The
task watchdog defaults to 5 ms and cannot be configured above 9 ms. Rust
configures the VSM stale/lost heartbeat thresholds to 5/9 ms; because the VSM
fault comparison is strict `>`, a 10 ms age is faulted. The VSM then applies
its safe-stop reaction. Rust also rejects invalid sensor values and returns
zero torque with disabled contactor commands. The Rust controller context and
input/output buffers use static storage; no heap allocation is used in the
step path.

The thread feeds Zephyr's task watchdog after processing the cycle. When the
board exposes a `watchdog0` alias, Zephyr's task-watchdog fallback requests a
10 ms hardware timeout. On task expiry, the callback latches safe state and
requests reboot. That is not a substitute for independent
de-energize-to-safe outputs: board-specific contactor/inverter disable wiring
and watchdog reset behavior must be validated independently.

In the current app the sensor path is deliberately invalid, so every step
returns safe outputs. No path transmits CAN frames or touches physical I/O.

## Migration boundary

- Integrated in Zephyr: `helios-core`, `helios-vsm`, and `helios-vdc`.
- Not integrated yet: `helios-energy` state estimation and thermal models,
  `helios-can` transport, board sensor/actuator drivers, or a vehicle protocol.
- Remains host software: digital twin, SIL adapters, tests, and developer tools.
- Remains Android software: AAOS/HMI modules. Zephyr does not replace Android.

Next hardware-specific work requires an exact NXP board/BSP and a reviewed
peripheral/safety contract: CAN controllers, ADC/GPIO assignments, HVIL and
isolation sensing, inverter command framing, reset states, timing/memory
budgets, independent interlocks, and test evidence. This scaffold is not
vehicle firmware release material, ASIL-certified, or authorization to energize
HV hardware.
