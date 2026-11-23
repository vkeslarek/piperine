//! The param types the interpreter runs (R1, PI-05): `f64` and `bool`. A param of another type is
//! the pin of a compiled body, so [`Interpreter::new`](crate::Interpreter::new) names it and refuses
//! the design instead of reading its bytes as a real.

use piperine_ir::{Body, ValueType};

/// The params of one body the interpreter cannot hold.
pub(crate) struct ParamTypes;

impl ParamTypes {
    /// One refusal per param of `body` whose type is neither `f64` nor `bool`, naming the param, its
    /// type and the transpiled backend that runs it (PI-05 AC 1).
    pub(crate) fn refused(body: Body<'_>) -> Vec<String> {
        body.params()
            .iter()
            .filter(|param| !matches!(param.ty, ValueType::F64 | ValueType::Bool))
            .map(|param| format!("body {} holds the param `{}` of type {}: the interpreter runs `f64` and `bool` params only (use the transpiled backend, `--backend rust`)", body.name(), param.name, param.ty))
            .collect()
    }
}
