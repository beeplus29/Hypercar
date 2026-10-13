//! Raw Cyclone DDS bindings. Use the safe `helios-dds` crate instead.
#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, dead_code, missing_debug_implementations)]
#![allow(clippy::all, unsafe_op_in_unsafe_fn)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
