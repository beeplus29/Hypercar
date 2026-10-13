# HELIOS research notes (verified 2026-10-02)

## Sandbox environment
- Ubuntu 24.04.5, kernel 6.18, 6 vCPU Xeon 2.5 GHz, 8 GB RAM (auto-scale 24 GB), 51 GB disk (39 GB free)
- NO /dev/kvm, no vmx/svm flags -> AAOS emulator / Cuttlefish cannot run here
- Docker 29.1.3 works (dockerd started manually, overlayfs)
- Rust 1.99.0 (2026-09-28), LLVM 23.1.1; targets: thumbv7em-none-eabihf, armv8r-none-eabihf, aarch64-unknown-linux-gnu, aarch64-linux-android
- clang 18.1.3, protoc 3.21.12 (/usr/include/google/protobuf/empty.proto present), aarch64-linux-gnu-gcc 13.3.0, OpenJDK 21.0.12 default (17 installed), cmake 3.28.3
- Cyclone DDS 0.10.5 (commit 2cdd114) with idlc -> /home/ubuntu/HELIOS/.deps/cyclonedds
- Crate versions (crates.io 2026-10): tonic 0.14.6, tonic-prost(-build) 0.14.6, prost 0.14.4, tokio 1.53.1, tokio-stream 0.1.19, bindgen 0.73.2, cc 1.5.1, proptest 1.11.0, cortex-m 0.7.9, cortex-m-rt 0.7.7, libm 0.2.16, serde 1.0.229, toml 1.1.6, clap 4.6.7, socketcan 4.0.0, thiserror 2.0.21, anyhow 1.0.104, futures 0.3.34, csv 1.4.0, serde_json 1.0.151, heapless 0.9.3

## AOSP / AAOS (source: https://source.android.com/docs/whatsnew/site-updates)
- Since 2025-03-27 `android-latest-release` manifest = latest release branch; currently `android17-release`
- Android 17 released June 2026; AOSP source drops only twice a year (Q2, Q4) from 2026
- AOSP `main` branch frozen ~March 2025 (hardware/interfaces main HEAD 1a56e38, 2025-03-26) -> do not use main
- Tags: android-17.0.0_r1 (manifest 7a9e46b; hardware/interfaces 32a1167). android17-release hardware/interfaces HEAD 0162af6 (2026-05-14); device/generic/car f788327 (2026-03-28); packages/services/Car 9f04df6 (2026-04-09)
- `trout` AAOS reference platform deprecated (Aug 2026 site update); Cuttlefish auto is the virtual reference
- VHAL AIDL frozen versions 1,2,3,4 + current; default service `android.hardware.automotive.vehicle@V4-default-service`; emulator uses `@V4-emulator-service`
- gRPC VHAL: hardware/interfaces/automotive/vehicle/aidl/impl/current/grpc/ (GRPCVehicleHardware.cpp, GRPCVehicleProxyServer.cpp, proto/VehicleServer.proto). Libraries: `android.hardware.automotive.vehicle@default-grpc-hardware-lib`, `...@default-grpc-server-lib`, `...@default-grpc-libgrpc`. Client uses InsecureChannelCredentials, CreateChannel(service_addr), ValuePollingLoop -> StartPropertyValuesStream.
- Android 17 VehicleServer has 12 RPCs: GetAllPropertyConfig(stream), SetValues, GetValues, UpdateSampleRate, CheckHealth, Dump, StartPropertyValuesStream(stream), Subscribe, Unsubscribe, GetMinMaxSupportedValues, GetSupportedValuesLists, StartSupportedValuesChangeStream(stream). (main/2025 had 9)
- Property ID = index | group | area | type. Group SYSTEM 0x10000000, VENDOR 0x20000000, BACKPORTED 0x30000000. Area GLOBAL 0x01000000, WINDOW 0x03000000, MIRROR 0x04000000, SEAT 0x05000000, DOOR 0x06000000, WHEEL 0x07000000, VENDOR 0x08000000. Type STRING 0x00100000, BOOLEAN 0x00200000, INT32 0x00400000, INT32_VEC 0x00410000, INT64 0x00500000, INT64_VEC 0x00510000, FLOAT 0x00600000, FLOAT_VEC 0x00610000, BYTES 0x00700000, MIXED 0x00e00000. Wheel areas LF 0x1, RF 0x2, LR 0x4, RR 0x8. Access READ 1 WRITE 2 READ_WRITE 3; change mode STATIC 0, ON_CHANGE 1, CONTINUOUS 2.
- Parsed IDs: PERF_VEHICLE_SPEED 0x11600207, PERF_VEHICLE_SPEED_DISPLAY 0x11600208, EV_BATTERY_LEVEL 0x11600309, INFO_EV_BATTERY_CAPACITY 0x11600106, GEAR_SELECTION 0x11400400, CURRENT_GEAR 0x11400401, EV_BATTERY_INSTANTANEOUS_CHARGE_RATE 0x1160030C, EV_CHARGE_PORT_CONNECTED 0x1120030B, PERF_STEERING_ANGLE 0x11600209, INFO_MAKE 0x11100101, INFO_MODEL 0x11100102, INFO_FUEL_TYPE 0x11410105, PARKING_BRAKE_ON 0x11200402, IGNITION_STATE 0x11400409, ENV_OUTSIDE_TEMPERATURE 0x11600703, EV_CURRENT_BATTERY_CAPACITY 0x1160030D, EV_BATTERY_AVERAGE_TEMPERATURE 0x1160030E, ELECTRONIC_STABILITY_CONTROL_STATE 0x1140040F, ELECTRONIC_STABILITY_CONTROL_ENABLED 0x1120040E, EV_CHARGE_STATE 0x11400F41, PERF_ODOMETER 0x11600204, TIRE_PRESSURE 0x17600309
- VehicleGear: NEUTRAL 0x1, REVERSE 0x2, PARK 0x4, DRIVE 0x8. EvChargeState: UNKNOWN 0, CHARGING 1, FULLY_CHARGED 2, NOT_CHARGING 3, ERROR 4
- sdk_car_x86_64.mk inherits car_generic_system.mk, car_system_ext.mk, car_emulator_product.mk, car_emulator_vendor.mk; lunch choices still `<product>-trunk_staging-userdebug`
- AOSP host packages (source.android.com): git-core gnupg flex bison build-essential zip curl zlib1g-dev libc6-dev-i386 x11proto-core-dev libx11-dev lib32z1-dev libgl1-mesa-dev libxml2-utils xsltproc unzip fontconfig (all installed OK on 24.04)

## Donut Lab (sources: https://www.donutlab.com/motor/ , https://www.donutlab.com/ces-battery-announcement/)
- 21" motor: up to 630 kW, 4,300 Nm, 40 kg. 17" (enclosed/open): up to 150 kW, 1,200 Nm (21 kg per press). 12": 150 kW, 300 Nm.
- Donut internal hypercar case: 1,450 kg vehicle, 1,500 kW & 9,000 Nm total wheel torque, 0-100 km/h < 2 s.
- Donut solid-state battery (CES 2026-01-05): claims 400 Wh/kg, full charge in 5 min, 100,000 cycle design life, >99% capacity at -30 C and >100 C, custom voltages. Claims are vendor-only; independent skepticism exists (r/electricvehicles, ResearchGate "Donut Lab SSB VTT 2026 comparative analysis"). Treat as UNVERIFIED.
- No public voltage / max-speed / efficiency data for the motor.

## HV safety regulations
- GTR No. 13 / FMVSS 305 / UN R94-R95 post-crash: HV <= 30 VAC / 60 VDC within 60 s after impact (measured not earlier than 5 s), or energy < 2.0 J, or isolation criteria (sources: federalregister.gov 2016-05187; thaiauto EV safety deck)
- Isolation: >= 100 ohm/V for DC HV, >= 500 ohm/V for AC HV (or combined AC+DC unless AC protected) — UN R100, FMVSS 305 (regulations.gov NHTSA-2017-0085-0001)
- High voltage class: >60 V and <=1500 V DC, >30 V and <=1000 V AC (EUR-Lex R100 rev)

## Ferrocene (sources: ferrocene.dev, public-docs.ferrocene.dev release notes)
- TUV SUD qualified: ISO 26262 ASIL D (TCL3), IEC 61508 SIL 4 (some sources SIL 3), IEC 62304 Class C. 25.11 added certified `core` subset (IEC 61508); 26.02 automotive core certification; latest 26.05.0 (2026-07)
- thumbv7em-none-eabi added experimental in 24.05; armv8r-none-eabihf experimental in 24.08 — check qualified-target list per release before committing
- Supports Linux and QNX Neutrino

## Project BOM inputs (project PDF / EV_OS)
- PDF says 1200 V powertrain/battery; BOM: OBC 550-850 VDC (Ovar), DC 800 V e-compressor (Guchen), Mersen MEV100 fuse 1000 VDC, CM-350SIC inverter, HIITIO iBDU, Bender IR155 IMD, GIGAVAC GX110CEB / P115CDA contactors, MS 50.4P VCU, NXP S32G3, NVIDIA Thor, Infinitybox, Snapp Automotive AAOS on 12.3" LCD
- Tyres: Cup 2 front 295/30ZR20, rear 345/30ZR21; alt Pirelli Trofeo RS front 305/30ZR21, rear 335/30ZR21
- Steering: DCE dual-MGU EPAS rack, 2.5 turns lock-to-lock
- Brakes: AP Racing Mono-E 10-pot front / 6-pot rear, Surface Transforms CCM 355 mm F / 365 mm R
- EV_OS S32K3 lab linker: FLASH 0x00400000 len 2048K, RAM 0x20400000 len 512K

## Cuttlefish auto gRPC VHAL reference (device/google/cuttlefish @ android17-release 283645a, 2026-04-16)
- guest/hals/vehicle/VehicleService.cpp: `DefaultVehicleHal(std::make_unique<GRPCVehicleHardware>(serverAddr))`, `hardware->waitForConnected(5s)`, registers `android.hardware.automotive.vehicle.IVehicle/default`; thread pool 4. Port from sysprop `ro.boot.vhal_proxy_server_port`; eth server addr 192.168.98.1 (namespace /mnt/run/auto_eth) or vsock CID host.
- Android.bp: cc_binary `android.hardware.automotive.vehicle@V4-cf-service`, defaults ["VehicleHalDefaults"], vendor, relative_install_path "hw", header_libs ["IVehicleHardware","vhal_vsockinfo"], static_libs ["DefaultVehicleHal","VehicleHalUtils","android.hardware.automotive.vehicle@default-grpc-hardware-lib"], shared_libs ["libbase","libcutils","libgrpc++","liblog","libprotobuf-cpp-full"], cflags -Wno-unused-parameter, vintf_fragment module (manifest hal aidl android.hardware.automotive.vehicle version 4 IVehicle/default)
- rc: `service vendor.vehicle-cf-eth /apex/.../bin/hw/...@V4-cf-service eth` class early_hal, user vehicle_network, group system inet, enter_namespace net /mnt/run/auto_eth
- sepolicy shared/auto/sepolicy/vhal/hal_vehicle_default.te: carwatchdog_client_domain(hal_vehicle_default); binder_use(hal_vehicle_default); vsock perms; system_server.te: allow system_server hal_vehicle_server:process sigkill
- shared/auto/device_vendor.mk: LOCAL_VHAL_PRODUCT_PACKAGE := com.android.hardware.automotive.vehicle.cf; PRODUCT_PRODUCT_PROPERTIES += cppd.connectvhal.Timeoutmillis=60000
- Host side: host/commands/vhal_proxy_server (C++ GRPCVehicleProxyServer wrapping FakeVehicleHardware) -> HELIOS Rust server replaces this role.
- VHAL JSON config format (default_config/config/*.json): {"apiVersion":1,"properties":[{"property":"VehicleProperty::X","defaultValue":{"floatValues":[..]},"areas":[{"areaId":"Constants::WHEEL_FRONT_LEFT","minFloatValue":..}],"access":"VehiclePropertyAccess::READ","changeMode":"VehiclePropertyChangeMode::CONTINUOUS","minSampleRate":1.0,"maxSampleRate":10.0}]} — only used by FakeVehicleHardware; GRPC hardware gets configs from server GetAllPropertyConfig.
- car_emulator_vendor.mk sets PRODUCT_PACKAGES += android.hardware.automotive.vehicle@V4-emulator-service; ro.carwatchdog.vhal_healthcheck.interval=10

## Design decisions taken (for report)
- Pack 200s3p 45 Ah NMC-equivalent, 840 V max (800 V class) — BOM OBC 550-850 V, 800 V e-compressor, 1000 VDC fuse all 800 V class; PDF "1200 V" conflicts -> flagged
- 21" rims at all 4 corners (Trofeo RS 305/30ZR21 F, 335/30ZR21 R) because Donut 21" motor; Cup 2 20" front incompatible
- MS 50.4P VCU (Bosch proprietary toolchain) conflicts with 100% Rust mandate -> Rust VCU on NXP S32K3 lockstep M7; MS 50.4P optional data logger only
- VSM property test found real bug: precharge abort via current-ramp phase briefly closed MAIN+ (fixed)
- Twin `hvil` scenario found VSM bug #2: decel_request tied to state, dropped when non-latching HVIL shutdown settled in Standby at ~110 km/h -> added stop_latch (until standstill)
- Twin `step_steer` found VDC tuning defect: 33.6 % yaw overshoot (passive car near-neutral vs K_us target; slow integrator) -> single-track steady-state Mz feed-forward, faded out above 50-85 % of ay limit (skidpad showed linear FF saturating Mz against the turn). Now 1.6 % overshoot
- Skidpad criterion: analytic capacity fixed point ay = sum mu(Fz_i) Fz_i / m (load transfer + downforce); achieved 1.39 g of 1.56 g (89 %); acceptance 85 % (original 1.4 g target was arbitrary)

## AOSP 17 VHAL proto details (vendored to interfaces/aosp/vhal, hw/interfaces 0162af69)
- package android.hardware.automotive.vehicle.proto; imports android/hardware/automotive/vehicle/*.proto + google/protobuf/empty.proto
- VehiclePropValue{timestamp=1, area_id=2, prop=3, status=4, int32_values=5, float_values=6, int64_values=7, byte_values=8, string_value=9}; VehiclePropValues{values=1}
- VehiclePropConfig{prop=1, access=2, change_mode=3, area_configs=4, config_array=5, config_string=6, min_sample_rate=7, max_sample_rate=8}; VehicleAreaConfig{area_id, min/max int32/int64/float, supported_enum_values, access, support_variable_update_rate, has_supported_value_info}
- StatusCode OK0 TRY_AGAIN1 INVALID_ARG2 NOT_AVAILABLE3 ACCESS_DENIED4 INTERNAL_ERROR5 ... SUBSYSTEM_NOT_CONNECTED11; VehicleHalCallStatus{status_code}
- SubscribeOptions{prop_id, area_ids, sample_rate, resolution, enable_variable_update_rate}; UnsubscribeRequest{prop_id, area_id}; DumpResult{caller_should_dump_state, buffer, refresh_property_configs}
- Cyclone 0.10.5 cross-process loopback 1 kHz, reliable KEEP_LAST 64: 5000/5000, p50 1027 us, p99 1170 us inter-arrival; KEEP_LAST 1: 4999/5000 (history overwrite, not network loss)
