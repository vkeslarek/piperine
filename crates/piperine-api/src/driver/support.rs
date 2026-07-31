//! Shared helpers every policy (optimize, center, and the black-box
//! engine) needs: reading a metric's raw value at the current operating
//! point, and resolving a design parameter's authored nominal to seed a
//! search's starting point (MD-34: `mod.rs` declares, this file
//! implements).

use crate::error::Error;

/// Read a metric's scalar value at the current operating point: a
/// declared margin name first (as [`crate::Session::gradient`] resolves
/// it), then a solved analog net name. Shared by every policy that needs
/// a raw (non-differentiated) objective/constraint readback — the
/// black-box engine (T18) and center's normalized-margin scoring (T19).
pub(crate) fn read_metric(op: &crate::results::OpResult, metric: &str) -> Result<f64, Error> {
    if let Ok(entry) = op.margins().get(metric) {
        return Ok(entry.scalar());
    }
    op.v(metric)
}

/// Resolve `label.param`'s AUTHORED (declared) default value by walking
/// the authored instance tree from `top` (MD-25 — never `flat_module`).
/// Used to seed a policy's starting point before any `Session::set` call
/// has moved it. Shared by every policy that needs a design-parameter
/// starting point.
pub(crate) fn nominal_param_value(
    design: &piperine_lang::pom::Design,
    top: &str,
    label: &str,
    param: &str,
) -> Result<f64, Error> {
    let top_module = design.module(top).ok_or_else(|| {
        Error::Measurement(format!("driver: top module `{top}` not found in the authored POM"))
    })?;
    find_param_default(design, top_module, "", label, param).ok_or_else(|| {
        Error::Measurement(format!(
            "driver: instance `{label}` param `{param}` not found on the authored instance tree"
        ))
    })
}

fn find_param_default(
    design: &piperine_lang::pom::Design,
    module: &piperine_lang::pom::Module,
    parent_path: &str,
    target_label: &str,
    param: &str,
) -> Option<f64> {
    use piperine_lang::value::Value;
    for instance in module.instances() {
        let path = if parent_path.is_empty() {
            instance.name().to_string()
        } else {
            format!("{parent_path}.{}", instance.name())
        };
        let Some(child) = design.module(instance.module_name()) else { continue };
        if path == target_label {
            return child
                .params()
                .iter()
                .find(|p| p.name() == param)
                .and_then(|p| p.default())
                .and_then(Value::as_real);
        }
        if let Some(v) = find_param_default(design, child, &path, target_label, param) {
            return Some(v);
        }
    }
    None
}
