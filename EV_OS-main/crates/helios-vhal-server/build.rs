fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "../../interfaces/aosp/vhal";
    
    // We need to point tonic-build to the root of the proto directory
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &["../../interfaces/aosp/vhal/VehicleServer.proto"],
            &[proto_dir],
        )?;
        
    Ok(())
}
