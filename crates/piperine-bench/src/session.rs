//! [`SimSession`] — owns one entry point's forked [`Design`] and runs
//! analyses against it: stage → elaborate-and-solve → snapshot. Nothing in
//! here is aware of the interpreter; [`crate::host::SimHost`] is the glue
//! between the two.

use std::rc::Rc;

use piperine_codegen::device::CircuitCompiler;
use piperine_lang::eval::Value;
use piperine_lang::Design;
use piperine_solver::solver::Context;

use crate::error::BenchError;
use crate::objects::OpResult;
use crate::waveform::{AcTrace, NoiseTrace, Trace};

/// Analysis configuration read from a `Solver` config-bundle value
/// (SPEC_BENCH.md §5.1) before an analysis runs.
#[derive(Debug, Clone)]
pub struct SolverConfig {
    pub temperature: f64,
    pub reltol: f64,
    pub abstol: f64,
    pub gmin: f64,
    pub max_iter: usize,
}

impl Default for SolverConfig {
    fn default() -> Self {
        let ctx = Context::default();
        Self {
            temperature: ctx.temperature,
            reltol: ctx.reltol,
            abstol: ctx.abstol,
            gmin: ctx.gmin,
            max_iter: ctx.max_iter,
        }
    }
}

impl SolverConfig {
    fn to_context(&self) -> Context {
        Context {
            temperature: self.temperature,
            reltol: self.reltol,
            abstol: self.abstol,
            gmin: self.gmin,
            max_iter: self.max_iter,
            ..Context::default()
        }
    }
}

/// One entry point's simulation session: a forked [`Design`] (own staging
/// area, SPEC_BENCH.md §9 isolation) rooted at the bench's module.
pub struct SimSession {
    design: Design,
    module: String,
}

impl SimSession {
    pub fn new(design: Design, module: String) -> Self {
        Self { design, module }
    }

    pub fn design(&self) -> &Design {
        &self.design
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    /// Stage a parameter override on the instance labeled `label` (or the
    /// bench's own module, for an empty label) — consumed by the next
    /// analysis (SPEC_BENCH.md §6.2/§7).
    pub fn stage(&self, label: &str, param: &str, value: piperine_lang::Value) {
        self.design.set_param(label, param, value);
    }

    /// Run a DC operating-point analysis (`$op`, SPEC_BENCH.md §5): apply
    /// staged overrides, re-elaborate to IR, build the circuit, and solve.
    /// Every analysis is a pure function of (design + staged overrides +
    /// config) — nothing here is remembered between calls. `nodeset`
    /// (SPEC_BENCH.md §5.1 `OpConfig.nodeset`) seeds the Newton initial
    /// guess.
    pub fn run_op(&self, config: &SolverConfig, nodeset: &Value) -> Result<OpResult, BenchError> {
        let applied = self.design.with_overrides_applied(&self.module)?;
        let ir = piperine_lang::ppr_to_ir(&applied)?;
        let mut compiler = CircuitCompiler::new(&ir);
        let (mut circuit, info) = compiler.build_circuit_mapped(&self.module)?;
        circuit.init_digital();
        circuit.rebuild_digital_topology();
        let ivs = build_ivs(&info, nodeset, circuit.netlist())?;
        let mut dc = circuit.dc(config.to_context())?;
        dc.apply_initial_conditions(ivs);
        let result = dc.solve()?;
        Ok(OpResult::new(result, Rc::new(info)))
    }

    /// Run a transient analysis (`$tran`, SPEC_BENCH.md §5): same
    /// elaborate-and-solve recipe as [`Self::run_op`], through
    /// `CircuitInstance::transient` instead of `::dc`. `step: None` (the
    /// config bundle's `step = 0.0` "auto") selects the adaptive stepper.
    /// `start` (SPEC_BENCH.md §5.1 `TranConfig.start`) is the earliest
    /// recorded time — the solver still integrates from t=0, but steps with
    /// `t < start` are dropped from the trace. `ic` (SPEC_BENCH.md §5.1
    /// `TranConfig.ic`) seeds the t=0 node voltages.
    pub fn run_tran(
        &self,
        stop: f64,
        step: Option<f64>,
        start: f64,
        config: &SolverConfig,
        ic: &Value,
    ) -> Result<Trace, BenchError> {
        let applied = self.design.with_overrides_applied(&self.module)?;
        let ir = piperine_lang::ppr_to_ir(&applied)?;
        let mut compiler = CircuitCompiler::new(&ir);
        let (mut circuit, info) = compiler.build_circuit_mapped(&self.module)?;
        circuit.init_digital();
        circuit.rebuild_digital_topology();
        let ivs = build_ivs(&info, ic, circuit.netlist())?;
        let opts = match step {
            Some(dt) => piperine_solver::analysis::transient::TransientAnalysisOptions::new(stop, dt),
            None => piperine_solver::analysis::transient::TransientAnalysisOptions::new_adaptive(stop, stop * 1e-3),
        }
        .with_record_from(start);
        let mut solver = circuit.transient(opts, config.to_context())?;
        solver.apply_initial_conditions(ivs);
        let result = solver.solve()?;
        Ok(Trace::new(result, Rc::new(info)))
    }

    /// Run an AC small-signal sweep (`$ac`, SPEC_BENCH.md §5). `Oct` scale
    /// maps to the solver's logarithmic sweep (it has lin/log only).
    pub fn run_ac(
        &self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<AcTrace, BenchError> {
        let applied = self.design.with_overrides_applied(&self.module)?;
        let ir = piperine_lang::ppr_to_ir(&applied)?;
        let mut compiler = CircuitCompiler::new(&ir);
        let (mut circuit, info) = compiler.build_circuit_mapped(&self.module)?;
        circuit.init_digital();
        circuit.rebuild_digital_topology();
        let opts = piperine_solver::analysis::ac::AcSweepAnalysisOptions {
            start_frequency: fstart,
            stop_frequency: fstop,
            steps: points,
            logarithmic,
        };
        let result = circuit.ac(config.to_context())?.solve_sweep(opts)?;
        Ok(AcTrace::new(result, Rc::new(info)))
    }

    /// Run an output-referred noise analysis (`$noise`, SPEC_BENCH.md §5).
    /// `out` and `reference` are net names resolved against the built
    /// circuit's net map (ground names map to the reference node).
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
        let applied = self.design.with_overrides_applied(&self.module)?;
        let ir = piperine_lang::ppr_to_ir(&applied)?;
        let mut compiler = CircuitCompiler::new(&ir);
        let (mut circuit, info) = compiler.build_circuit_mapped(&self.module)?;
        circuit.init_digital();
        circuit.rebuild_digital_topology();
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
        Ok(NoiseTrace::new(result))
    }
}

/// Resolve a bench-visible net name to a solver node identifier. Ground
/// names (`gnd`/`GND`/`vss`/`VSS`) map to the reference node; anything else
/// is looked up in the built circuit's net map.
fn resolve_net(
    info: &piperine_codegen::device::CircuitBuildInfo,
    name: &str,
) -> Result<piperine_solver::analog::NodeIdentifier, BenchError> {
    if matches!(name, "gnd" | "GND" | "vss" | "VSS") {
        return Ok(piperine_solver::analog::NodeIdentifier::Gnd);
    }
    info.nets.get(name).cloned().ok_or_else(|| {
        BenchError::Measurement(format!("net `{name}` is not addressable"))
    })
}

/// Build solver initial-value hints from an `ic`/`nodeset` `Map<Net, Real>`
/// (SPEC_BENCH.md §5.1). Keys are `NetRef`s resolved through the built
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
            let net_name = match k {
                Value::Object(obj) => obj
                    .as_any()
                    .downcast_ref::<crate::objects::NetRef>()
                    .map(|n| n.name.clone())
                    .ok_or_else(|| {
                        BenchError::Measurement(format!(
                            "ic/nodeset keys must be Nets, got {}",
                            k.type_name()
                        ))
                    })?,
                other => {
                    return Err(BenchError::Measurement(format!(
                        "ic/nodeset keys must be Nets, got {}",
                        other.type_name()
                    )))
                }
            };
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
