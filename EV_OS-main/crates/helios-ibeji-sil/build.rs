fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // Cargo build scripts run before dependent crates; pin protoc to a local
    // vendored binary so building the SIL adapter needs no system installation.
    unsafe { std::env::set_var("PROTOC", protoc) };
    tonic_prost_build::configure().compile_protos(
        &[
            "proto/invehicle_digital_twin.proto",
            "proto/digital_twin_provider.proto",
            "proto/digital_twin_consumer.proto",
        ],
        &["proto"],
    )?;
    println!("cargo:rerun-if-changed=proto");
    Ok(())
}
