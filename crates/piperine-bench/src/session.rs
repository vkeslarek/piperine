//! [`SimSession`] — owns one entry point's forked [`Design`] and runs
//! analyses against it: stage → elaborate-and-solve → snapshot. Nothing in
//! here is aware of the interpreter; [`crate::host::SimHost`] is the glue
//! between the two.

use std::rc::Rc;

use piperine_codegen::device::CircuitCompiler;
use piperine_lang::eval::Value;
use piperine_lang::Design;
use piperine_solver::solver::{Context, Policy};

use crate::error::BenchError;
use crate::objects::OpResult;
use crate::waveform::{AcTrace, NoiseTrace, Trace};

/// Analysis configuration read from a `Solver` config-bundle value
/// (piperine-bench/docs/SPEC.md §5.1) before an analysis runs.
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
        let tol = piperine_solver::solver::Tolerances::default();
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
    fn to_context(&self) -> Context {
        Context {
            tolerances: piperine_solver::solver::Tolerances {
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
    fn to_policy(&self) -> Policy {
        Policy {
            max_iter: self.max_iter,
            dc_damp_tolerance: self.dc_damp_tolerance,
        }
    }
}

/// One entry point's simulation session: a forked [`Design`] (own staging
/// area, piperine-bench/docs/SPEC.md §9 isolation) rooted at the bench's module.
pub struct SimSession {
    design: Design,
    module: String,
    /// Builds `@device`-annotated instances (SPEC Part VI §7).
    provider: Option<std::rc::Rc<dyn piperine_codegen::device::DeviceProvider>>,
    /// Lifecycle hooks + plugin bench tasks (SPEC Part VI §8/§6).
    plugins: Option<std::rc::Rc<dyn crate::plugins::BenchPlugins>>,
}

impl SimSession {
    pub fn new(design: Design, module: String) -> Self {
        Self { design, module, provider: None, plugins: None }
    }

    /// Wire a plugin host as the device provider for this session's builds.
    pub fn set_device_provider(
        &mut self,
        provider: std::rc::Rc<dyn piperine_codegen::device::DeviceProvider>,
    ) {
        self.provider = Some(provider);
    }

    /// Wire a plugin host's hooks and bench tasks into this session.
    pub fn set_plugins(&mut self, plugins: std::rc::Rc<dyn crate::plugins::BenchPlugins>) {
        self.plugins = Some(plugins);
    }

    /// Plugin bench-task dispatch, for [`crate::host::SimHost::syscall`].
    pub(crate) fn plugin_task(
        &self,
        name: &str,
        args: Vec<piperine_lang::eval::Value>,
    ) -> Option<Result<piperine_lang::eval::Value, piperine_lang::eval::EvalError>> {
        self.plugins.as_ref()?.run_bench_task(name, args)
    }

    /// Fire `after_solve` (hook 7).
    fn fire_after_solve(
        &self,
        analysis: &str,
        node_voltages: &[(String, f64)],
    ) -> Result<(), BenchError> {
        if let Some(p) = &self.plugins {
            p.after_solve(analysis, node_voltages).map_err(BenchError::Plugin)?;
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
    /// bench's own module, for an empty label) — consumed by the next
    /// analysis (piperine-bench/docs/SPEC.md §6.2/§7).
    pub fn stage(&self, label: &str, param: &str, value: piperine_lang::Value) {
        self.design.set_param(label, param, value);
    }

    /// Run a DC operating-point analysis (`$op`, piperine-bench/docs/SPEC.md §5): apply
    /// staged overrides, lower to resolved bodies, build the circuit, and solve.
    /// Every analysis is a pure function of (design + staged overrides +
    /// config) — nothing here is remembered between calls. `nodeset`
    /// (piperine-bench/docs/SPEC.md §5.1 `OpConfig.nodeset`) seeds the Newton initial
    /// guess.
    fn build_circuit(&self) -> Result<(piperine_solver::core::circuit::CircuitInstance, piperine_codegen::device::CircuitBuildInfo), BenchError> {
        // Hook 3 (`transform_design`): plugins stage their mutations, then
        // the pure re-elaboration below consumes them like any bench write.
        if let Some(p) = &self.plugins {
            p.transform_design(&self.design).map_err(BenchError::Plugin)?;
        }
        let applied = self.design.with_overrides_applied(&self.module)?;
        // Hook 4 (`before_lower`): read-only view of the applied design.
        if let Some(p) = &self.plugins {
            p.before_lower(&applied).map_err(BenchError::Plugin)?;
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

    pub fn run_op(&self, config: &SolverConfig, nodeset: &Value) -> Result<OpResult, BenchError> {
        let (mut circuit, info) = self.build_circuit()?;
        let ivs = build_ivs(&info, nodeset, circuit.netlist())?;
        let mut dc = circuit.dc(config.to_context())?;
        dc.policy = config.to_policy();
        dc.apply_initial_conditions(ivs);
        let result = dc.solve()?;
        drop(dc);
        let digital = Self::snapshot_digital(&info, &circuit);
        let voltages: Vec<(String, f64)> = info
            .nets
            .iter()
            .map(|(name, node)| {
                let v = if *node == piperine_solver::analog::NodeIdentifier::Gnd {
                    0.0
                } else {
                    result.get_node(node).unwrap_or(0.0)
                };
                (name.clone(), v)
            })
            .collect();
        self.fire_after_solve("op", &voltages)?;
        Ok(OpResult::new(result, digital, Rc::new(info)))
    }

    /// The top module's digital net values as reals (0/1; X/Z read as NaN so
/// an assertion on an undriven net fails loud, never silently passes).
fn snapshot_digital(
        info: &piperine_codegen::device::CircuitBuildInfo,
    circuit: &piperine_solver::core::circuit::CircuitInstance,
) -> std::collections::HashMap<String, f64> {
    use piperine_solver::digital::LogicValue;
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

/// Run a transient analysis (`$tran`, piperine-bench/docs/SPEC.md §5): same
    /// elaborate-and-solve recipe as [`Self::run_op`], through
    /// `CircuitInstance::transient` instead of `::dc`. `step: None` (the
    /// config bundle's `step = 0.0` "auto") selects the adaptive stepper.
    /// `start` (piperine-bench/docs/SPEC.md §5.1 `TranConfig.start`) is the earliest
    /// recorded time — the solver still integrates from t=0, but steps with
    /// `t < start` are dropped from the trace. `ic` (piperine-bench/docs/SPEC.md §5.1
    /// `TranConfig.ic`) seeds the t=0 node voltages.
    pub fn run_tran(
        &self,
        stop: f64,
        step: Option<f64>,
        start: f64,
        config: &SolverConfig,
        ic: &Value,
    ) -> Result<Trace, BenchError> {
        let (mut circuit, info) = self.build_circuit()?;
        let ivs = build_ivs(&info, ic, circuit.netlist())?;
        let opts = match step {
            // SPICE is always adaptive; `.step` is the initial dt for the
            // PI controller. `step = 0` (the "auto" sentinel) seeds dt at
            // stop/1000. Output interpolation onto the print grid is a
            // follow-up (ROADMAP).
            Some(dt) if dt > 0.0 => {
                piperine_solver::analysis::transient::TransientAnalysisOptions::new(stop, dt)
            }
            _ => piperine_solver::analysis::transient::TransientAnalysisOptions::new(stop, stop * 1e-3),
        }
        .with_record_from(start);
        let mut solver = circuit.transient(opts, config.to_context())?;
        solver.policy = config.to_policy();
        solver.apply_initial_conditions(ivs);
        let result = solver.solve()?;
        self.fire_after_solve("tran", &[])?;
        Ok(Trace::new(result, Rc::new(info)))
    }

    /// Run an AC small-signal sweep (`$ac`, piperine-bench/docs/SPEC.md §5). `Oct` scale
    /// maps to the solver's logarithmic sweep (it has lin/log only).
    pub fn run_ac(
        &self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<AcTrace, BenchError> {
        let (mut circuit, info) = self.build_circuit()?;
        let opts = piperine_solver::analysis::ac::AcSweepAnalysisOptions {
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

    /// Run an output-referred noise analysis (`$noise`, piperine-bench/docs/SPEC.md §5).
    /// `out` and `reference` are net names resolved against the built
    /// circuit's net map (ground names map to the reference node).
    #[allow(clippy::too_many_arguments)]
    pub fn run_noise(
        &self,
        out: &str,
        reference: &str,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<NoiseTrace, BenchError> {
        let (mut circuit, info) = self.build_circuit()?;
        let out = resolve_net(&info, out)?;
        let reference = resolve_net(&info, reference)?;
        let opts = piperine_solver::analysis::noise::NoiseAnalysisOptions {
            sweep_options: piperine_solver::analysis::ac::AcSweepAnalysisOptions {
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

/// Resolve a bench-visible net name to a solver node identifier
/// ([`NetLookup`] with this crate's measurement error).
fn resolve_net(
    info: &piperine_codegen::device::CircuitBuildInfo,
    name: &str,
) -> Result<piperine_solver::analog::NodeIdentifier, BenchError> {
    use crate::objects::NetLookup;
    info.net_node(name)
        .ok_or_else(|| BenchError::Measurement(format!("net `{name}` is not addressable")))
}

/// Build solver initial-value hints from an `ic`/`nodeset` `Map<Net, Real>`
/// (piperine-bench/docs/SPEC.md §5.1). Keys are `NetRef`s resolved through the built
/// circuit's net map; values are `Real`. Ground keys and unresolved nets are
/// skipped (ground has no index).
fn build_ivs(
    info: &piperine_codegen::device::CircuitBuildInfo,
    map: &Value,
    netlist: &piperine_solver::analog::Netlist,
) -> Result<Vec<piperine_solver::math::iv::InitialValue<piperine_solver::analog::AnalogReference, f64>>, BenchError> {
    use piperine_solver::analog::AnalogVariable;
    use piperine_solver::math::iv::InitialValue;
    let mut ivs = Vec::new();
    if let Value::Map(entries) = map {
        for (k, v) in entries.borrow().iter() {
            let net_name = crate::objects::NetRef::from_value(k)
                .map(|n| n.name.clone())
                .ok_or_else(|| {
                    BenchError::Measurement(format!(
                        "ic/nodeset keys must be Nets, got {}",
                        k.type_name()
                    ))
                })?;
            let node = resolve_net(info, &net_name)?;
            let value = match v {
                Value::Real(r) => *r,
                Value::Nat(n) => *n as f64,
                Value::Int(n) => *n as f64,
                other => {
                    return Err(BenchError::Measurement(format!(
                        "ic/nodeset values must be Real, got {}",
                        other.type_name()
                    )))
                }
            };
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
