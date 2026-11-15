//! The backend a twin runs on: the interpreter, or the design transpiled to Rust, built by
//! `rustc` and loaded (feature `ir-rust`, RS-07). The host names it; nothing picks one by a
//! heuristic. [`Backend::parse`] is pure over the variable's value, so no test sets one.

/// Which backend builds a twin's circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// `piperine-interp`: the reference.
    Interp,
    /// `piperine-transpile`: the IR printed as Rust, compiled and loaded.
    Rust,
}

impl Backend {
    /// The variable that picks one backend for a run.
    pub const VARIABLE: &'static str = "PIPERINE_BACKEND";

    /// The name a rejection and the variable use.
    pub fn name(&self) -> &'static str {
        match self {
            Backend::Interp => "interp",
            Backend::Rust => "rust",
        }
    }

    /// The backends the variable's value selects: unset or empty gives both, in the order
    /// interpreter then Rust; `interp` or `rust` gives that one only; anything else is an error
    /// naming the two valid values.
    pub fn parse(value: Option<&str>) -> Result<Vec<Backend>, String> {
        match value.filter(|v| !v.is_empty()) {
            None => Ok(vec![Backend::Interp, Backend::Rust]),
            Some("interp") => Ok(vec![Backend::Interp]),
            Some("rust") => Ok(vec![Backend::Rust]),
            Some(other) => Err(format!("{}={other} is not a backend; the valid values are `interp` and `rust`", Self::VARIABLE)),
        }
    }

    /// The backends of this process: `PIPERINE_BACKEND` read once through [`Backend::parse`].
    /// An invalid value fails the test that asks, naming the two valid values.
    pub fn selected() -> Vec<Backend> {
        let value = std::env::var(Self::VARIABLE).ok();
        Self::parse(value.as_deref()).unwrap_or_else(|message| panic!("{message}"))
    }
}
