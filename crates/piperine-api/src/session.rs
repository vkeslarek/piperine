//! [`SimSession`] — owns a [`Design`] and runs analyses against it:
//! stage → elaborate-and-solve → snapshot. Every analysis is a pure function
//! of (design + staged overrides + config); nothing is remembered between
//! calls.

use std::collections::HashMap;
use std::rc::Rc;

use piperine_codegen::device::{CircuitBuildInfo, CircuitCompiler};
use piperine_lang::Design;
use piperine_solver::prelude::{Context, Policy};

use crate::error::Error;
use crate::results::{NetLookup, OpResult};
use crate::waveform::{AcTrace, NoiseTrace, Trace};

/// Analysis configuration (tolerances + convergence tunables) read before an
/// analysis runs.
#[derive(Debug, Clone)]
pub struct SolverConfig {
    pub temperature: f64,
    pub reltol: f64,
    pub abstol: f64,
    pub gmin: f64,
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
}

impl Default for SolverConfig {
    fn default() -> Self {
        let tol = piperine_solver::prelude::Tolerances::default();
        let policy = Policy::default();
        Self {
            temperature: tol.temperature,
            reltol: tol.reltol,
            abstol: tol.abstol,
            gmin: tol.gmin,
            max_iter: policy.max_iter,
            dc_damp_tolerance: policy.dc_damp_tolerance,
        }
    }
}

impl SolverConfig {
    /// The shared solver [`Context`] (tolerances) this config maps to.
    /// Public: hosts that drive `CircuitInstance` analyses directly (the
    /// Python live session) reuse the same mapping.
    pub fn to_context(&self) -> Context {
        Context {
            tolerances: piperine_solver::prelude::Tolerances {
                temperature: self.temperature,
                reltol: self.reltol,
                abstol: self.abstol,
                gmin: self.gmin,
                ..Default::default()
            },
        }
    }

    /// The convergence tunables (MD-04): set on each analysis solver so
    /// user `max_iter` / `dc_damp_tolerance` reach the Newton loop.
    /// Public for the same host reuse as [`Self::to_context`].
    pub fn to_policy(&self) -> Policy {
        Policy {
            max_iter: self.max_iter,
            dc_damp_tolerance: self.dc_damp_tolerance,
        }
    }
}

/// A simulation session over one design + top module: staging area,
/// elaborate-and-solve analyses, result snapshots.
pub struct SimSession {
    design: Design,
    module: String,
    /// Builds `@device`-annotated instances (SPEC Part VI §7).
    provider: Option<Rc<dyn piperine_codegen::device::DeviceProvider>>,
    /// Lifecycle hooks (SPEC Part VI §8) fired around builds and solves.
    hooks: Option<Rc<dyn crate::hooks::SimHooks>>,
}

impl SimSession {
    pub fn new(design: Design, module: String) -> Self {
        Self { design, module, provider: None, hooks: None }
    }

    /// Wire a plugin host as the device provider for this session's builds.
    pub fn set_device_provider(
        &mut self,
        provider: Rc<dyn piperine_codegen::device::DeviceProvider>,
    ) {
        self.provider = Some(provider);
    }

    /// Wire the lifecycle hooks (a plugin host) into this session.
    pub fn set_hooks(&mut self, hooks: Rc<dyn crate::hooks::SimHooks>) {
        self.hooks = Some(hooks);
    }

    fn fire_after_solve(&self, analysis: &str, node_voltages: &[(String, f64)]) -> Result<(), Error> {
        if let Some(h) = &self.hooks {
            h.after_solve(analysis, node_voltages).map_err(Error::Plugin)?;
        }
        Ok(())
    }

    pub fn design(&self) -> &Design {
        &self.design
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    /// Stage a parameter override on the instance labeled `label` (or the
    /// session's own module, for an empty label) — consumed by the next
    /// analysis.
    pub fn stage(&self, label: &str, param: &str, value: piperine_lang::Value) {
        self.design.set_param(label, param, value);
    }

    /// Apply staged overrides, lower to resolved bodies, build the circuit.
    fn build_circuit(&self) -> Result<(piperine_solver::prelude::CircuitInstance, CircuitBuildInfo), Error> {
        // `transform_design`: hooks stage their mutations, then the pure
        // re-elaboration below consumes them like any staged write.
        if let Some(h) = &self.hooks {
            h.transform_design(&self.design).map_err(Error::Plugin)?;
        }
        let applied = self.design.with_overrides_applied(&self.module)?;
        // `before_lower`: read-only view of the applied design.
        if let Some(h) = &self.hooks {
            h.before_lower(&applied).map_err(Error::Plugin)?;
        }
        let bodies = piperine_codegen::ir::lower_bodies(&applied)?;
        let mut compiler = CircuitCompiler::new(&applied, &bodies);
        if let Some(provider) = &self.provider {
            compiler = compiler.with_device_provider(provider.as_ref());
        }
        let (mut circuit, info) = compiler.build_circuit_mapped(&self.module)?;
        circuit.init_digital()?;
        circuit.rebuild_digital_topology();
        Ok((circuit, info))
    }

    /// Run a DC sensitivity analysis (`.sens`): `∂V(output)/∂(param)` at the
    /// operating point for every requested `(label, param)` pair, by central
    /// finite difference over the compile-once restamp path. Outputs are
    /// host-visible net names; results key by `(output, "label.param")`.
    /// Loud on unknown nets/elements/params and on parameters whose write
    /// would rebuild the circuit.
    pub fn run_sens(
        &self,
        outputs: &[&str],
        params: &[(String, String)],
        dp_rel: f64,
        config: &SolverConfig,
    ) -> Result<crate::results::SensResult, Error> {
        use crate::results::NetLookup;
        let (mut circuit, info) = self.build_circuit()?;
        // Resolve host names → solver `Net`s (keyed back to the host name
        // after the solve — solver-side labels are internal ids).
        let mut nets = Vec::with_capacity(outputs.len());
        for name in outputs {
            let node = info.net_node(name).ok_or_else(|| {
                Error::Measurement(format!("net `{name}` is not addressable"))
            })?;
            let var = piperine_solver::abi::AnalogVariable::Node(node);
            let net = circuit
                .nets()
                .into_iter()
                .find(|n| n.analog_variable().map(|v| **v == var).unwrap_or(false))
                .ok_or_else(|| {
                    Error::Measurement(format!("net `{name}` is not a solved analog net"))
                })?;
            nets.push(((*name).to_string(), net));
        }
        let opts = piperine_solver::prelude::SensAnalysisOptions {
            outputs: nets.iter().map(|(_, n)| n.clone()).collect(),
            params: params.to_vec(),
            dp_rel,
        };
        let mut solver = circuit.sens(opts, config.to_context())?;
        solver.policy = config.to_policy();
        let inner = solver.solve()?;
        let mut d = std::collections::HashMap::new();
        for (name, net) in &nets {
            for (label, param) in params {
                if let Some(v) = inner.get(net.label(), label, param) {
                    d.insert((name.clone(), format!("{label}.{param}")), v);
                }
            }
        }
        self.fire_after_solve("sens", &[])?;
        Ok(crate::results::SensResult { d })
    }

    /// Run a periodic-steady-state analysis (single shooting): one converged
    /// period `t ∈ [tstab, tstab+period]` as a transient trace, plus the
    /// shooting stats. The drive period is user-supplied; non-periodic
    /// circuits, wrong periods, and digital `k·T` dividers fail loud.
    pub fn run_pss(
        &self,
        period: f64,
        tstab: f64,
        config: &SolverConfig,
    ) -> Result<crate::results::PssResult, Error> {
        let (mut circuit, info) = self.build_circuit()?;
        let opts = piperine_solver::prelude::PssAnalysisOptions::new(period).with_tstab(tstab);
        let mut solver = circuit.pss(opts, config.to_context())?;
        solver.policy = config.to_policy();
        let inner = solver.solve()?;
        self.fire_after_solve("pss", &[])?;
        Ok(crate::results::PssResult {
            trace: crate::waveform::Trace::new(inner.trace, Rc::new(info)),
            stats: inner.stats,
        })
    }

    /// Run a DC operating-point analysis. `nodeset` (net name → volts) seeds
    /// the Newton initial guess.
    pub fn run_op(
        &self,
        config: &SolverConfig,
        nodeset: Option<&HashMap<String, f64>>,
    ) -> Result<OpResult, Error> {
        let (mut circuit, info) = self.build_circuit()?;
        let ivs = build_ivs(&info, nodeset, circuit.netlist())?;
        let mut dc = circuit.dc(config.to_context())?;
        dc.policy = config.to_policy();
        dc.apply_initial_conditions(ivs);
        let result = dc.solve()?;
        drop(dc);
        let digital = Self::snapshot_digital(&info, &circuit);
        self.fire_after_solve("op", &node_voltages(&info, &result))?;
        Ok(OpResult::new(result, digital, Rc::new(info)))
    }

    /// Compile-once DC sweep (MD-18): elaborate/JIT the circuit **once**,
    /// then for each value restamp `label.param` on the already-compiled
    /// circuit through the solver's [`CircuitInstance::set_element_param`]
    /// path and re-run the operating point. Never re-elaborates or re-JITs
    /// per point — that is an architecture defect, not a perf tweak.
    ///
    /// Returns one [`OpResult`] per value, in order. Each result's build
    /// info carries the point's parameter value so device-internal current
    /// recomputation (`.i(a, b)` on force-less two-terminal devices) reads
    /// the swept value, not the build-time one.
    pub fn run_op_sweep(
        &self,
        label: &str,
        param: &str,
        values: &[f64],
        config: &SolverConfig,
        nodeset: Option<&HashMap<String, f64>>,
    ) -> Result<Vec<OpResult>, Error> {
        let (mut circuit, mut info) = self.build_circuit()?;
        let mut results = Vec::with_capacity(values.len());
        for &v in values {
            circuit.set_element_param(
                label,
                param,
                piperine_solver::abi::Value::Real(v),
            )?;
            // Mirror the restamp into the build info: `.i()` recomputes a
            // force-less two-terminal current from kernel + params.
            if let Some(inst) = info.instances.iter_mut().find(|i| i.label == label)
                && let Some(pidx) = inst.kernel.param_names().iter().position(|n| n == param)
            {
                inst.params[pidx] = v;
            }
            let ivs = build_ivs(&info, nodeset, circuit.netlist())?;
            let mut dc = circuit.dc(config.to_context())?;
            dc.policy = config.to_policy();
            dc.apply_initial_conditions(ivs);
            let result = dc.solve()?;
            drop(dc);
            let digital = Self::snapshot_digital(&info, &circuit);
            self.fire_after_solve("op", &node_voltages(&info, &result))?;
            results.push(OpResult::new(result, digital, Rc::new(info.clone())));
        }
        Ok(results)
    }

    /// The top module's digital net values as reals (0/1; X/Z read as NaN so
    /// an assertion on an undriven net fails loud, never silently passes).
    /// Public: hosts that drive `CircuitInstance` directly (the Python live
    /// session) build the same [`OpResult`] digital snapshot.
    pub fn snapshot_digital(
        info: &CircuitBuildInfo,
        circuit: &piperine_solver::prelude::CircuitInstance,
    ) -> HashMap<String, f64> {
        use piperine_solver::prelude::LogicValue;
        info.digital_nets
            .iter()
            .map(|(name, &idx)| {
                let v = match circuit.digital_state.nets.get(idx) {
                    Some(LogicValue::Zero) => 0.0,
                    Some(LogicValue::One) => 1.0,
                    _ => f64::NAN,
                };
                (name.clone(), v)
            })
            .collect()
    }

    /// Run a transient analysis: same elaborate-and-solve recipe as
    /// [`Self::run_op`], through `CircuitInstance::transient` instead of
    /// `::dc`. `step: None` selects the adaptive stepper. `start` is the
    /// earliest recorded time — the solver still integrates from t=0, but
    /// steps with `t < start` are dropped from the trace. `ic` (net name →
    /// volts) seeds the t=0 node voltages.
    pub fn run_tran(
        &self,
        stop: f64,
        step: Option<f64>,
        start: f64,
        config: &SolverConfig,
        ic: Option<&HashMap<String, f64>>,
    ) -> Result<Trace, Error> {
        let (mut circuit, info) = self.build_circuit()?;
        let ivs = build_ivs(&info, ic, circuit.netlist())?;
        let opts = match step {
            // SPICE is always adaptive; `step` is the initial dt for the
            // PI controller. `step = 0` (the "auto" sentinel) seeds dt at
            // stop/1000. Output interpolation onto the print grid is a
            // follow-up (ROADMAP).
            Some(dt) if dt > 0.0 => {
                piperine_solver::prelude::TransientAnalysisOptions::new(stop, dt)
            }
            _ => piperine_solver::prelude::TransientAnalysisOptions::new(stop, stop * 1e-3),
        }
        .with_record_from(start);
        let mut solver = circuit.transient(opts, config.to_context())?;
        solver.policy = config.to_policy();
        solver.apply_initial_conditions(ivs);
        let result = solver.solve()?;
        self.fire_after_solve("tran", &[])?;
        Ok(Trace::new(result, Rc::new(info)))
    }

    /// Run an AC small-signal sweep.
    pub fn run_ac(
        &self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<AcTrace, Error> {
        let (mut circuit, info) = self.build_circuit()?;
        let opts = piperine_solver::prelude::AcSweepAnalysisOptions {
            start_frequency: fstart,
            stop_frequency: fstop,
            steps: points,
            logarithmic,
        };
        let mut ac = circuit.ac(config.to_context())?;
        ac.policy = config.to_policy();
        let result = ac.solve_sweep(opts)?;
        self.fire_after_solve("ac", &[])?;
        Ok(AcTrace::new(result, Rc::new(info)))
    }

    /// Run an output-referred noise analysis. `out` and `reference` are net
    /// names resolved against the built circuit's net map (ground names map
    /// to the reference node).
    pub fn run_noise(
        &self,
        out: &str,
        reference: &str,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<NoiseTrace, Error> {
        let (mut circuit, info) = self.build_circuit()?;
        let out = resolve_net(&info, out)?;
        let reference = resolve_net(&info, reference)?;
        let opts = piperine_solver::prelude::NoiseAnalysisOptions {
            sweep_options: piperine_solver::prelude::AcSweepAnalysisOptions {
                start_frequency: fstart,
                stop_frequency: fstop,
                steps: points,
                logarithmic,
            },
            output_node: out,
            reference_node: reference,
            input_source_name: None,
        };
        let result = circuit.noise(opts, config.to_context())?.solve()?;
        self.fire_after_solve("noise", &[])?;
        Ok(NoiseTrace::new(result))
    }
}

/// Resolve a host-visible net name to a solver node identifier.
fn resolve_net(
    info: &CircuitBuildInfo,
    name: &str,
) -> Result<piperine_solver::prelude::NodeIdentifier, Error> {
    info.net_node(name)
        .ok_or_else(|| Error::Measurement(format!("net `{name}` is not addressable")))
}

/// The solved node voltages as `(net name, volts)` pairs — the payload the
/// `after_solve` hook observes for operating-point analyses.
fn node_voltages(
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
fn build_ivs(
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
