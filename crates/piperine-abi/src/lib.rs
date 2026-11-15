//! The Piperine device ABI (`.specs/features/device-abi/abi.md`): the
//! `#[repr(C)]` contract a device links against (`raw`), the shipped layouts
//! with their byte rules (`layout`), the owned shape builder (`shape`), the
//! shape-local checks of abi.md §9 (`validate`) and the Rust side of §7: the
//! `Device` trait and its lenses (`device`). The crate depends on no other
//! crate; `unsafe` lives only in `raw`, `thunk` and `caller`.

pub mod device;
pub mod layout;
pub mod raw;
pub mod shape;
pub mod validate;
