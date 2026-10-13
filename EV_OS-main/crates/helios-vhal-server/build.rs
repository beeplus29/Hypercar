fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // Use the platform-matched compiler so VHAL generation needs no system protoc.
    unsafe { std::env::set_var("PROTOC", protoc) };
    let proto_dir = "../../interfaces/aosp/vhal";
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["../../interfaces/aosp/vhal/VehicleServer.proto"], &[proto_dir])?;

    Ok(())
}
