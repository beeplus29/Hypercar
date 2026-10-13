//! Embed an rpath to the Cyclone DDS lib dir exported by helios-dds-sys (links = "ddsc").
fn main() {
    if let Ok(dir) = std::env::var("DEP_DDSC_LIB_DIR") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
    }
}
