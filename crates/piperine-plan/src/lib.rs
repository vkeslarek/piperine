//! What the two backends of an `Ir` share (feature `ir-rust`, MD-63): the plan of a body (its ABI
//! shape and what each function of that shape computes) and the circuit of a design, built over
//! whatever supplies the module of each body. The interpreter (`piperine-interp`) and the
//! transpiler (`piperine-transpile`) read one plan, so pins, read order, partial order and affine
//! flags cannot drift between them.

mod array;
mod composite;
mod derive;
mod digital;
mod netlist;
mod pin;
mod plan;
mod resolved;

pub use array::ArrayPin;
pub use composite::{CompositeLayout, CompositeLeaf};
pub use derive::{Binding, DerivePlan, ParamReads};
pub use digital::{CarrierLayout, ReactionMode, ReactionPlan, RecordLayout, StatementPlan};
pub use netlist::{Modules, Netlist};
pub use pin::PinLayout;
pub use plan::{BodyPlan, DelayedPins, FunctionPlan, Mode, Pins, ResolvedPins, Source, Target, Term};
pub use resolved::DriveLayout;
