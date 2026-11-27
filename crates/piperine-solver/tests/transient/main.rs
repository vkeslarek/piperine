//! The transient run of the solver on devices and scripted sides: restarts and reentry, truncation error,
//! checkpoint and restore, landing on boundaries, jumps and crossings, the stepper fold, the step observer
//! and its statistics, the normative failure rules, limiting, settled values, the settle subscriber,
//! mixed-signal sources, the operating point, temperature, the environment, array windows, the device
//! lifecycle and the device library against analytic values. Each former target is a module of this one.

mod array_windows;
mod checkpoint_restore;
mod crossing_rules;
mod digital_settle_subscriber;
mod environment;
mod failure_rules;
mod helper_devices;
mod jump_retake;
mod jump_step;
mod landing_rule;
mod lifecycle;
mod limiting_report;
mod mixed_signal;
mod operating_point;
mod settled_value;
mod step_observer;
mod step_statistics;
mod step_fold;
mod temperature;
mod transient_reentry;
mod truncation;
