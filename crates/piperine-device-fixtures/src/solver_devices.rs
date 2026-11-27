//! The devices of the solver's integration targets, one module per target (`tests/<group>/<target>.rs`), each
//! holding the hand-written devices that target builds its circuits from, with the shapes they share.

pub mod abi_ac;
pub mod abi_dc;
pub mod abi_noise;
pub mod abi_sensitivity;
pub mod abi_tran;
pub mod array_windows;
pub mod bind;
pub mod checkpoint_restore;
pub mod crossing_rules;
pub mod digital_settle_subscriber;
pub mod environment;
pub mod failure_rules;
pub mod host_set;
pub mod jump_retake;
pub mod lifecycle;
pub mod limiting_report;
pub mod live_params;
pub mod noise_report;
pub mod parity_baseline;
pub mod probe_selection;
pub mod protocol_crossings;
pub mod protocol_real;
pub mod protocol_transient;
pub mod pz_device_fault;
pub mod sensitivity_fallback;
pub mod settled_value;
pub mod solver_entry;
pub mod temperature;
pub mod truncation;
