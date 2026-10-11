cmake_minimum_required(VERSION 3.20.0)

set(KCONFIG_ROOT "${CMAKE_CURRENT_LIST_DIR}/Kconfig")
find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(helios_control)

set(HELIOS_ROOT "${CMAKE_CURRENT_LIST_DIR}/../..")
set(HELIOS_FFI_MANIFEST "${HELIOS_ROOT}/crates/helios-zephyr-control-ffi/Cargo.toml")
set(HELIOS_FFI_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo")

if(NOT RUST_TARGET)
  if(BOARD MATCHES "^native_sim")
    execute_process(
      COMMAND rustc -vV
      OUTPUT_VARIABLE RUSTC_VERSION
      RESULT_VARIABLE RUSTC_RESULT
      OUTPUT_STRIP_TRAILING_WHITESPACE
    )
    if(NOT RUSTC_RESULT EQUAL 0)
      message(FATAL_ERROR "rustc is required to build the HELIOS Rust control library")
    endif()
    string(REGEX MATCH "host: ([^\n\r]+)" RUST_HOST_MATCH "${RUSTC_VERSION}")
    set(RUST_TARGET "${CMAKE_MATCH_1}")
  else()
    message(FATAL_ERROR
      "Set -DRUST_TARGET=<Rust target triple> for this board. "
      "No S32G GoldBox/GreenBox Rust target is assumed or configured.")
  endif()
endif()

set(HELIOS_FFI_ARCHIVE
  "${HELIOS_FFI_TARGET_DIR}/${RUST_TARGET}/firmware/libhelios_zephyr_control_ffi.a")

add_custom_command(
  OUTPUT "${HELIOS_FFI_ARCHIVE}"
  COMMAND "${CMAKE_COMMAND}" -E env
          "CARGO_TARGET_DIR=${HELIOS_FFI_TARGET_DIR}"
          cargo build --manifest-path "${HELIOS_FFI_MANIFEST}"
                --profile firmware --target "${RUST_TARGET}"
  DEPENDS
    "${HELIOS_ROOT}/Cargo.toml"
    "${HELIOS_ROOT}/Cargo.lock"
    "${HELIOS_FFI_MANIFEST}"
    "${HELIOS_ROOT}/crates/helios-zephyr-control-ffi/src/lib.rs"
    "${HELIOS_ROOT}/crates/helios-core/src/lib.rs"
    "${HELIOS_ROOT}/crates/helios-vsm/src/lib.rs"
    "${HELIOS_ROOT}/crates/helios-vdc/src/lib.rs"
  WORKING_DIRECTORY "${HELIOS_ROOT}"
  VERBATIM
)

add_custom_target(helios_rust_control DEPENDS "${HELIOS_FFI_ARCHIVE}")

target_sources(app PRIVATE src/main.c)
target_include_directories(app PRIVATE include)
add_dependencies(app helios_rust_control)
target_link_libraries(app PRIVATE "${HELIOS_FFI_ARCHIVE}")
