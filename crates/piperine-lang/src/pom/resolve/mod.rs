//! Name resolution: the stages that run after lowering and write only into
//! resolution slots and the diagnostics list. Each stage computes its answers
//! from the immutable design into an [`Output`], then the answers are applied.

mod behaviors;
mod classify;
mod context;
mod imports;
mod leaves;
mod members;
mod name_resolution;
mod names;
mod output;
mod scopes;
mod shape;

use context::Ctx;
pub(super) use name_resolution::NameResolution;
use output::Output;
