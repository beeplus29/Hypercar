//! Build: run Cyclone `idlc` on idl/helios/*.idl, compile the generated C type
//! support, and generate Rust bindings for `dds/dds.h` + the generated headers.
//!
//! CYCLONEDDS_HOME must point at a Cyclone DDS 0.10.5 install prefix
//! (default set in .cargo/config.toml to <workspace>/.deps/cyclonedds).

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let home = PathBuf::from(env::var("CYCLONEDDS_HOME").expect("CYCLONEDDS_HOME not set (run tools/bootstrap.sh)"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let idl_dir = manifest.join("../../idl/helios");
    let idl = idl_dir.join("Telemetry.idl");
    println!("cargo:rerun-if-changed={}", idl.display());
    println!("cargo:rerun-if-env-changed=CYCLONEDDS_HOME");

    let idlc = home.join("bin/idlc");
    let st = Command::new(&idlc)
        .env("LD_LIBRARY_PATH", home.join("lib"))
        .arg("-o")
        .arg(&out)
        .arg(&idl)
        .status()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", idlc.display()));
    assert!(st.success(), "idlc failed on {}", idl.display());

    cc::Build::new()
        .file(out.join("Telemetry.c"))
        .include(home.join("include"))
        .include(&out)
        .flag_if_supported("-Wno-unused-parameter")
        .compile("helios_idl");

    println!("cargo:rustc-link-search=native={}", home.join("lib").display());
    println!("cargo:rustc-link-lib=dylib=ddsc");
    println!("cargo:lib_dir={}", home.join("lib").display());

    let header = out.join("wrapper.h");
    std::fs::write(&header, "#include \"dds/dds.h\"\n#include \"Telemetry.h\"\n").expect("write wrapper.h");
    let bindings = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_arg(format!("-I{}", home.join("include").display()))
        .clang_arg(format!("-I{}", out.display()))
        .allowlist_function("dds_.*")
        .allowlist_type("helios_.*|dds_.*")
        .allowlist_var("helios_.*|DDS_.*")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .layout_tests(true)
        .generate()
        .expect("bindgen failed");
    bindings.write_to_file(out.join("bindings.rs")).expect("write bindings");
}
