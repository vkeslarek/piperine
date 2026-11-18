//! What the two backends of an `Ir` share (feature `ir-rust`, MD-63): the plan of a body (its ABI
//! shape and what each function of that shape computes) and the circuit of a design, built over
//! whatever supplies the module of each body. The interpreter (`piperine-interp`) and the
//! transpiler (`piperine-transpile`) read one plan, so pins, read order, partial order and affine
//! flags cannot drift between them.

mod derive;
mod digital;
mod netlist;
mod plan;

pub use derive::{Binding, DerivePlan};
pub use digital::{CarrierLayout, ReactionMode, ReactionPlan, RecordLayout, StatementPlan};
pub use netlist::{Modules, Netlist};
pub use plan::{BodyPlan, DelayedPins, FunctionPlan, Mode, Pins, Source, Target, Term};
