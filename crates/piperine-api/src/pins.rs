//! `Pins`: where a written param lands. A run writes through its bound circuit; a description
//! (`Session::describe`) binds no circuit and writes nowhere, yet applies a host value through the same
//! code (`Session::set_param`, `set_leaf`, `set_text_param`: the checks, the recompute of what a
//! placement derives from it, the table), so what it shows is what a run reads (HII-10).

use piperine_solver::prelude::{CircuitInstance, ParamValue};

/// The device pins a param write reaches.
pub(crate) trait Pins {
    /// Whether a pin holds the param `param` of `instance`.
    fn holds(&self, instance: &str, param: &str) -> bool;

    /// Checks `value` against the param's own `where` without writing it; the refusal as text.
    fn check(&self, instance: &str, param: &str, value: &ParamValue) -> Result<(), String>;

    /// Writes `value` to the pin of `param` of `instance`; the refusal as text.
    fn write(&mut self, instance: &str, param: &str, value: ParamValue) -> Result<(), String>;
}

impl Pins for CircuitInstance {
    fn holds(&self, instance: &str, param: &str) -> bool {
        self.bound().names.params.contains_key(&(instance.to_string(), param.to_string()))
    }

    fn check(&self, instance: &str, param: &str, value: &ParamValue) -> Result<(), String> {
        self.check_set(instance, param, value).map_err(|error| error.to_string())
    }

    fn write(&mut self, instance: &str, param: &str, value: ParamValue) -> Result<(), String> {
        self.set(instance, param, value).map_err(|error| error.to_string())
    }
}

/// No circuit: a description. No pin holds a param, so nothing is checked against a pin and nothing is written.
#[derive(Debug, Default)]
pub(crate) struct Unbound;

impl Pins for Unbound {
    fn holds(&self, _instance: &str, _param: &str) -> bool {
        false
    }

    fn check(&self, _instance: &str, _param: &str, _value: &ParamValue) -> Result<(), String> {
        Ok(())
    }

    fn write(&mut self, _instance: &str, _param: &str, _value: ParamValue) -> Result<(), String> {
        Ok(())
    }
}
