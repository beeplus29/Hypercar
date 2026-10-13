# HELIOS Ibeji-compatible SIL adapter

This crate provides a **local protocol-compatible test harness**, not the
upstream Eclipse Ibeji runtime:

- a loopback-only in-vehicle digital-twin registry implementing `Register` and
  `FindById`;
- a read-only telemetry provider implementing the Ibeji provider gRPC service;
- subscriber delivery through the Ibeji consumer `Publish` RPC.

The protocol definitions in `proto/` are based on the public
[Eclipse Ibeji in-vehicle digital twin interface](https://github.com/eclipse-ibeji/ibeji/tree/main/interfaces/invehicle_digital_twin/v1)
and sample provider/consumer interfaces under the same repository's
`samples/interfaces/sample_grpc/v1`. The upstream project is MIT-licensed.
This crate is not maintained by, endorsed by, or a distribution of Eclipse
Ibeji.

The simulated registry is intentionally not advertised as an implementation of
the full Ibeji service: it does not implement Chariott discovery, DTDL model
validation, property/command modules, persistence, authentication, TLS, or the
rest of the Ibeji ecosystem. The provider rejects `Set` and `Invoke` requests,
because SIL exposes observation-only telemetry and no actuator commands.

Run the complete local path from the workspace root:

```sh
cargo run -p helios-twin --features sil-ibeji --bin helios-twin-ibeji -- \
  --duration-s 10 --throttle 0.2
```

The command starts ephemeral loopback registry, provider, and consumer services;
registers the telemetry entity; subscribes; and sends simulation snapshots
through the gRPC publish flow. No hardware is required or accessed.
