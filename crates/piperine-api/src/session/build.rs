//! The build plumbing both session types share: the options a compilation
//! takes ([`BuildOptions`]), the one build recipe ([`build_circuit`]), and the
//! name/value plumbing an analysis needs on the way in and out (net
//! resolution, initial values, probe selection, the param mirror).

use std::collections::HashMap;
use std::rc::Rc;

use piperine_codegen::device::{CircuitBuildInfo, CircuitCompiler};
use piperine_lang::Design;

use crate::error::Error;
use crate::results::NetLookup;

/// The build-time options a [`Session`] is compiled with, kept together so
/// the compiled session can reproduce its own build (`Session::rebuild`).
#[derive(Clone)]
pub(super) struct BuildOptions {
    /// Builds `@device`-annotated instances (SPEC Part VI §7).
    pub(super) provider: Option<Rc<dyn piperine_codegen::device::DeviceProvider>>,
    /// Lifecycle hooks (SPEC Part VI §8) fired around builds and solves.
    pub(super) hooks: Option<Rc<dyn crate::hooks::SimHooks>>,
    /// Whether the compiled kernels include the `.disto` 2nd/3rd-derivative
    /// set. `true` by default — `CircuitCompiler::new`'s own default, and
    /// what makes [`Session::disto`] usable straight after a plain
    /// [`Session::compile`]. Those kernels are a real per-branch-combination
    /// Cranelift cost, so a caller that will never run `.disto` on this
    /// circuit opts out with [`SessionBuilder::disto`]`(false)`.
    pub(super) disto: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self { provider: None, hooks: None, disto: true }
    }
}

/// The one build recipe: consume `design`'s staged overrides, lower to
/// resolved bodies, JIT the circuit. The hook order is part of the contract —
/// `transform_design` (the host's chance to stage its own mutations) →
/// overrides consumed → `before_lower` (read-only, on the applied design) →
/// lower → compile. Returns the applied design alongside the build so the
/// session can restage against it later.
pub(super) fn build_circuit(
    design: &Design,
    module: &str,
    opts: &BuildOptions,
) -> Result<(piperine_solver::prelude::CircuitInstance, CircuitBuildInfo, Design), Error> {
    if let Some(h) = &opts.hooks {
        h.transform_design(design).map_err(Error::Plugin)?;
    }
    let applied = design.with_overrides_applied(module)?.fork();
    if let Some(h) = &opts.hooks {
        h.before_lower(&applied).map_err(Error::Plugin)?;
    }
    let bodies = piperine_codegen::resolve::lower_bodies(&applied)?;
    let mut compiler = CircuitCompiler::new(&applied, &bodies).with_disto(opts.disto);
    if let Some(provider) = &opts.provider {
        compiler = compiler.with_device_provider(provider.as_ref());
    }
    let (mut circuit, info) = compiler.build_circuit_mapped(module)?;
    circuit.init_digital()?;
    circuit.rebuild_digital_topology();
    Ok((circuit, info, applied))
}

/// Mirror a parameter write into the build info: `.i(a, b)` on a force-less
/// two-terminal device recomputes the branch current from kernel + params, so
/// a restamp the info does not see reports the pre-write current. The single
/// copy of this mirror — every restamp path (`Session::set`,
/// `Session::set_or_rebuild`, `Session::dc`, `Session::tran`'s scheduled
/// writes, and the staged sweep) goes through it.
pub(super) fn mirror_param(info: &mut CircuitBuildInfo, label: &str, param: &str, value: f64) {
    if let Some(inst) = info.instances.iter_mut().find(|i| i.label == label)
        && let Some(pidx) = inst.kernel.param_names().iter().position(|n| n == param)
    {
        inst.params[pidx] = value;
    }
}

/// Build a [`piperine_solver::prelude::ProbeSelection`] from `"instance.name"`
/// paths (HOST-08's `tran(probe = [...])`). Malformed paths (no `.`) fail
/// loud here; unknown device/observable pairs fail loud at solver setup
/// (ABI-35, `CircuitInstance::transient`).
pub(super) fn build_probe_selection(
    probe: &[&str],
) -> Result<piperine_solver::prelude::ProbeSelection, Error> {
    let mut selection = piperine_solver::prelude::ProbeSelection::new();
    for &path in probe {
        let (label, name) = crate::results::split_probe_path(path)?;
        selection = selection.request(label, name);
    }
    Ok(selection)
}

/// Resolve a host-visible net name to a solver node identifier.
pub(super) fn resolve_net(
    info: &CircuitBuildInfo,
    name: &str,
) -> Result<piperine_solver::prelude::NodeIdentifier, Error> {
    info.net_node(name)
        .ok_or_else(|| Error::Measurement(format!("net `{name}` is not addressable")))
}

/// The solved node voltages as `(net name, volts)` pairs — the payload the
/// `after_solve` hook observes for operating-point analyses.
pub(super) fn node_voltages(
    info: &CircuitBuildInfo,
    result: &piperine_solver::prelude::DcAnalysisResult,
) -> Vec<(String, f64)> {
    info.nets
        .iter()
        .map(|(name, node)| {
            let v = if *node == piperine_solver::prelude::NodeIdentifier::Gnd {
                0.0
            } else {
                result.get_node(node).unwrap_or(0.0)
            };
            (name.clone(), v)
        })
        .collect()
}

/// Build solver initial-value hints from a net-name → volts map. Keys
/// resolve through the built circuit's net map; ground keys are skipped
/// (ground has no index).
pub(super) fn build_ivs(
    info: &CircuitBuildInfo,
    map: Option<&HashMap<String, f64>>,
    netlist: &piperine_solver::prelude::Netlist,
) -> Result<Vec<piperine_solver::abi::InitialValue<piperine_solver::abi::AnalogReference, f64>>, Error> {
    use piperine_solver::abi::{AnalogVariable, InitialValue};
    let mut ivs = Vec::new();
    if let Some(map) = map {
        for (name, &value) in map {
            let node = resolve_net(info, name)?;
            if let Some(reference) = netlist.reference_for(&AnalogVariable::Node(node)) {
                ivs.push(InitialValue {
                    reference: reference.clone(),
                    value,
                });
            }
        }
    }
    Ok(ivs)
}

