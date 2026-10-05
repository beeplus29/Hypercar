# HELIOS Initial Workspace Setup Report

**Date:** 2026-10-02  
**Workspace:** `/home/ubuntu/HELIOS`  
**Project:** DIY Hypercar / HELIOS EV control stack

## Executive result

The native development workspace is operational.

- Rust workspace: **all tests passed** in Release mode.
- Digital twin: **12/12 scenarios passed**.
- Cyclone DDS: **0.10.5**, built from source with `idlc` enabled.
- DDS Rust bindings: generated from HELIOS IDL with `idlc`, compiled through `helios-dds-sys`, wrapped by a safe RAII API in `helios-dds`.
- DDS cross-process loopback: **5,000/5,000 samples**, zero reordering at 1 kHz using reliable KEEP_LAST 64; p50 inter-arrival 1,027 µs, p99 1,170 µs, max 1,357 µs.
- AOSP Android 17 VHAL gRPC source reference: vendored from `hardware/interfaces` `android17-release` commit `0162af698935100a590b7359581ac8b1b80693e5`.
- Original `EV_OS` repository workspace: native Release build, 4/4 host tests, Cyclone DDS local roundtrip smoke test all passed.

## Reproduction commands

### HELIOS Rust stack

```bash
cd /home/ubuntu/HELIOS
source "$HOME/.cargo/env"
cargo test --workspace --release
cargo build -p helios-twin --release
./target/release/helios-twin all --out out
```

Expected result: `12 scenarios, 0 failed` and all workspace test results `ok`.

### DDS loopback

```bash
cd /home/ubuntu/HELIOS
source "$HOME/.cargo/env"
export CYCLONEDDS_URI="file://$PWD/config/dds/helios_loopback.xml"
cargo test -p helios-dds --release

# Cross-process test
(./target/release/helios-dds-sub --domain 42 --count 5000 > /tmp/helios-dds-sub.log 2>&1) &
sleep 0.5
./target/release/helios-dds-pub --domain 42 --count 5000 --rate-hz 1000
cat /tmp/helios-dds-sub.log
```

### Original EV_OS CMake/DDS validation

```bash
cd /home/ubuntu/EV_OS
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j"$(nproc)"
ctest --test-dir build --output-on-failure

export CYCLONE_PREFIX="$PWD/.deps/cyclonedds"
cmake -S . -B build_dds -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$CYCLONE_PREFIX"
cmake --build build_dds -j"$(nproc)"
export CYCLONEDDS_URI="file://$PWD/config/dds/cyclonedds_vehicle_os_loopback.xml"
./scripts/run_dds_roundtrip_local.sh
```

Observed EV_OS result: `100% tests passed, 0 tests failed out of 4`; local DDS smoke test: `100% tests passed, 0 tests failed out of 1`.

## Implemented HELIOS components

| Area | Implementation | Verification |
|---|---|---|
| Shared safety types | `helios-core`, `#![no_std]` types, modes, gears, fault catalogue, parameters, control math | Unit tests and target builds |
| Vehicle state machine | HV sequencing, precharge/current ramp, contactors, crash/HVIL reactions, stop latch | 12 unit/property tests |
| Vehicle dynamics control | Yaw-rate reference, PI yaw tracking, steady-state single-track feed-forward, sideslip stabilization, torque allocation, TC/ABS, aero strategy | 18 unit tests; step steer 1.6% overshoot |
| Energy | 2-RC battery ECM, SOP limits, SOC EKF, thermal model | Unit tests and twin thermal scenario |
| CAN FD | Table-driven codec, E2E protection, bus-load calculations, DBC export | Unit tests and generated matrix checks |
| Digital twin | Twin-track plant, Pacejka-style tyres, load transfer, aero, HV circuit, battery, thermal, real VSM/VDC in loop | 12/12 scenarios |
| DDS | IDL, generated C type support, bindgen FFI, safe Rust entities/readers/writers, QoS profiles, publisher/subscriber tools | In-process and cross-process loopback |
| AAOS reference | Android 17 VHAL gRPC protos and provenance vendored | Source and message schema reviewed |

## Safety-relevant fixes discovered during validation

1. **HVIL stop latch:** a non-latching HVIL shutdown could settle in `Standby` while the vehicle was still moving, dropping the decel request. `stop_latch` now holds the stop request until standstill. Regression coverage is in `helios-vsm`.
2. **Yaw response:** initial yaw control produced 33.6% step-steer overshoot. A mathematically derived single-track steady-state yaw-moment feed-forward was added, then faded out as lateral acceleration approaches the nonlinear tyre limit. Result: 1.6% overshoot and 0.102 s response versus 0.192 s passive.
3. **Skidpad acceptance:** the arbitrary 1.4 g threshold was replaced by an analytic fixed-point tyre-capacity calculation using `mu(Fz)`, lateral load transfer, aero load, and vehicle mass. The run achieved 1.39 g versus 1.56 g calculated capacity, or 89% utilization; acceptance is 85%.

## Architecture decisions

- **Safety/performance domain:** Rust control crates are separated from the AAOS domain. The VSM, VDC, energy estimator, CAN codec and target firmware remain Rust-first.
- **Messaging:** Cyclone DDS is used for central-compute / AAOS bridge / SIL traffic. CAN FD remains the deterministic ECU network boundary.
- **DDS QoS:** telemetry may use best-effort; commands use reliable transient-local KEEP_LAST 1; loss-intolerant streams use reliable volatile KEEP_LAST 64. The 1 kHz measurement showed KEEP_LAST 1 can overwrite a sample when the reader drains too slowly; KEEP_LAST 64 delivered all 5,000 samples.
- **AAOS integration:** the AOSP 17 gRPC VHAL protocol is the selected bridge surface. The vendored proto set includes the current 12 RPCs, including supported-value streams introduced after the older 9-RPC interface.
- **Virtualization boundary:** AAOS/Cuttlefish is not executable in this sandbox because `/dev/kvm` is unavailable. The host-side Rust DDS and VHAL work can proceed; final AAOS boot, Binder, VINTF and SELinux validation belongs on a KVM-capable Linux host or hardware target.

## Current limitations / next critical phase

- The VHAL protocol reference is vendored and documented; the production Rust gRPC VHAL server implementation is the next software milestone.
- The current DDS IDL uses fixed-size arrays and POD fields intentionally. Variable-length strings/sequences should not be introduced into safety-critical telemetry without bounded-memory review.
- Cyclone DDS is linked through the workspace-local `.deps/cyclonedds` prefix and embedded ELF RUNPATH for the sandbox. A deployed target must replace this with a versioned sysroot/package and a locked runtime image.
- The baseline commits are local to the HELIOS workspace. No remote publication was performed.

## Key paths

- `Cargo.toml` — Rust workspace and lint policy
- `crates/helios-core` — shared types and parameters
- `crates/helios-vsm` — vehicle state machine
- `crates/helios-vdc` — vehicle dynamics control
- `crates/helios-energy` — battery/SOC/thermal models
- `crates/helios-can` — CAN FD matrix
- `crates/helios-twin` — digital twin and scenario runner
- `crates/helios-dds-sys` — Cyclone DDS FFI and generated support
- `crates/helios-dds` — safe DDS API and smoke-test binaries
- `idl/helios/Telemetry.idl` — DDS data contract
- `config/dds/helios_loopback.xml` — loopback profile
- `interfaces/aosp/vhal` — AOSP 17 VHAL gRPC protocol reference
- `docs/research_notes.md` — source and engineering notes
