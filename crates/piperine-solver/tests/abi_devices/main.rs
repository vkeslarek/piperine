//! The analog device library of the solver's ABI tests on the `Device` trait (SAI-11 and after): the
//! circuits of `Device`s read the analytic values the models state through DC, transient, AC, noise and
//! sensitivity, the surface the devices expose, and every hand-written device passes the partials verifier.
//! Each former target is a module of this one.

mod abi_ac;
mod abi_dc;
mod abi_devices;
mod abi_noise;
mod abi_sensitivity;
mod abi_surface;
mod abi_tran;
