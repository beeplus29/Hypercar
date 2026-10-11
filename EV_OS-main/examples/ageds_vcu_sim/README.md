# AGEDS controller simulation reference

This is an **offline, host-side simulation example**, not deployable vehicle
control software. It has no RTOS integration, sensor drivers, actuator drivers,
calibrated vehicle model, or safety qualification. Its command values are
normalized simulation signals only; do not connect them to hardware.

The controller accepts a timestamped snapshot containing IMU, ride-height,
infrared tire-temperature, inverter-temperature, vehicle-speed, steering,
throttle, and brake data. It rejects stale, invalid, or non-finite input instead
of treating a dropout as valid data. A mutex protects the controller's filtered
state and last command. `kControlPeriodMs` defines the intended 10 ms (100 Hz)
call period; the calling scheduler must enforce that cadence.

The small finite-candidate horizon search is a pedagogical MPC-shaped surrogate,
not a validated predictive vehicle model. The corner-entry cue estimates lateral
acceleration using a bicycle-model proxy and steering input, then compares that
prediction with measured lateral acceleration; it cannot establish real chassis
roll timing. The inverter override demonstrates priority handling by requesting
maximum simulated fan command and open louvers above a configurable threshold.
Thresholds, assumed wheelbase, and normalized outputs are illustrative only.

Build and run the host-side checks:

```sh
cmake -S . -B build
cmake --build build
ctest --test-dir build --output-on-failure
```
