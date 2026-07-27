//! The navigable object model: load a PHDL design, walk its modules, and read
//! back the ports/nets/instances/params/behaviors the author wrote (CLA-17).
//!
//! This is the api-canonical model (MD-27 §1): the Python binding's `_Design`/
//! `_Module`/`_InstanceView` are thin delegations onto the types declared here,
//! so both hosts navigate one model in two languages (MD-22).
//!
//! **What the model shows is the authored hierarchy** (MD-25): `Design::modules`
//! walks `instance → submodule → sub-instances` exactly as written, monomorphized
//! variants named but never collapsed. The flattened side artifact codegen builds
//! for itself is not part of this surface and no accessor here reads it.
//!
//! **Single-threaded by construction**: the model holds `Rc<piperine_lang::Design>`
//! rather than `Arc`, because the POM's interior (its staging area) is not `Sync` —
//! the same reason the Python wrappers are `unsendable`. Making the POM `Send` is a
//! separate, larger decision.

mod descriptors;

pub use descriptors::{Behavior, Instance, Net, Param, Port, Terminal};
pub use piperine_solver::prelude::{
    ModelDescriptor, ObservableDescriptor, ParamDescriptor, TerminalDescriptor,
};
