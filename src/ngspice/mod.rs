mod callbacks;
mod structs;
mod engine;

pub use engine::NgSpiceEngine;
pub use structs::Event;

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    clippy::all
)]
mod ffi {
    // Assuming bindings are generated correctly
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
