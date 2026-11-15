//! The Piperine device ABI (`.specs/features/device-abi/abi.md`): the
//! `#[repr(C)]` contract a device links against (`raw`) and the shipped
//! layouts with their byte rules (`layout`), and the owned shape builder
//! (`shape`). The crate depends on no other crate.

pub mod layout;
pub mod raw;
pub mod shape;
