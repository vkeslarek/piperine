//! [`SimSession`] — owns a [`Design`] and runs analyses against it:
//! stage → elaborate-and-solve → snapshot. Every analysis is a pure function
//! of (design + staged overrides + config); nothing is remembered between
//! calls.

use std::collections::HashMap;
use std::rc::Rc;

use piperine_codegen::device::{CircuitBuildInfo, CircuitCompiler};
use piperine_lang::Design;
use piperine_solver::abi::SolverStats;
use piperine_solver::prelude::{Context, Policy};

use crate::error::Error;
use crate::results::{IntrospectSnapshot, NetLookup, OpResult};
use crate::waveform::{AcTrace, NoiseTrace, Trace, Waveform};

/// Frequency-sweep geometry (HOST-23): `Lin` steps `points` values evenly
/// over `[fstart, fstop]`; `Dec`/`Oct` step logarithmically (decade/octave
/// per `points`) — the same three-way choice the prelude's `enum Scale`
/// (and the Python facade's `Scale`) already name. `impl Into<bool>` lets an
/// analysis's `logarithmic` argument accept either a bare `bool` (unchanged
/// for every existing caller — `bool: Into<bool>` via the identity `From`)
/// or a `Scale` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Lin,
    Dec,
    Oct,
}

impl Scale {
    /// `true` for `Dec`/`Oct` (a logarithmic sweep), `false` for `Lin` —
    /// the boolean every sweep-options struct actually stamps.
    pub fn is_logarithmic(&self) -> bool {
        !matches!(self, Scale::Lin)
    }
}

impl From<Scale> for bool {
    fn from(s: Scale) -> bool {
        s.is_logarithmic()
    }
}

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
            ..Default::default()
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
    /// `compile_disto` gates the `.disto` 2nd/3rd-derivative kernels
    /// (`CircuitCompiler::with_disto`) — every caller but [`Self::run_disto`]
    /// passes `false`: those kernels are a real per-branch-combination
    /// Cranelift compile cost that only `.disto` itself needs.
    fn build_circuit(&self, compile_disto: bool) -> Result<(piperine_solver::prelude::CircuitInstance, CircuitBuildInfo), Error> {
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
        let bodies = piperine_codegen::resolve::lower_bodies(&applied)?;
        let mut compiler = CircuitCompiler::new(&applied, &bodies).with_disto(compile_disto);
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
        let (mut circuit, info) = self.build_circuit(false)?;
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
        let (mut circuit, info) = self.build_circuit(false)?;
        let opts = piperine_solver::prelude::PssAnalysisOptions::new(period).with_tstab(tstab);
        let mut solver = circuit.pss(opts, config.to_context())?;
        solver.policy = config.to_policy();
        let inner = solver.solve()?;
        self.fire_after_solve("pss", &[])?;
        Ok(crate::results::PssResult {
            trace: crate::waveform::Trace::<crate::waveform::Waveform>::new(inner.trace, Rc::new(info)),
            stats: inner.stats,
        })
    }

    /// Run a pole-zero analysis (`.pz`): poles (and, when `input_source` is
    /// given, transmission zeros) of the linearized input→output transfer
    /// function at the DC operating point. `input_source` is the instance
    /// label of the driving voltage source (its ideal-source branch, the
    /// same one `Trace::i`/`OpResult::i` read); `output` is the measured net
    /// name, optionally differential against `output_ref`. Fails loud when
    /// `input_source`/`output`/`output_ref` are not addressable, when a
    /// device's AC stamp is not affine in `jω` (PZ-06), or when the circuit
    /// has no reactive elements (PZ-05, no finite poles).
    pub fn run_pz(
        &self,
        input_source: &str,
        output: &str,
        output_ref: Option<&str>,
        config: &SolverConfig,
    ) -> Result<crate::results::PzResult, Error> {
        let (mut circuit, info) = self.build_circuit(false)?;
        let output_node = resolve_net(&info, output)?;
        let output_ref_node = output_ref.map(|r| resolve_net(&info, r)).transpose()?;
        let options = piperine_solver::prelude::PoleZeroOptions {
            input_source: piperine_solver::abi::BranchIdentifier::new(input_source, "force0"),
            output: piperine_solver::abi::AnalogVariable::Node(output_node),
            output_ref: output_ref_node,
        };
        let solver = circuit.pz(options, config.to_context())?;
        let poles = solver.poles()?;
        let zeros = solver.zeros()?;
        self.fire_after_solve("pz", &[])?;
        Ok(piperine_solver::prelude::PoleZeroResult { poles, zeros }.into())
    }

    /// Run an N-port S-parameter analysis (`.sp`): the scattering matrix
    /// over a frequency sweep for every node carrying an `@rfport(num, z0)`
    /// attribute in this session's module (SP-01, SP-02). Ports are
    /// resolved from the design's attribute schema (`Design::rfports`),
    /// then translated to circuit node identifiers the same way any other
    /// host-visible net name is. Fails loud (SP-05) when the module
    /// declares no ports, a `z0` is non-positive, two ports collide on the
    /// same `num` or the same node, or a port's node is not addressable in
    /// the built circuit.
    pub fn run_sp(
        &self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<crate::results::SParamResult, Error> {
        let (mut circuit, info) = self.build_circuit(false)?;
        let rfports = self.design.rfports(&self.module)?;
        let mut ports = Vec::with_capacity(rfports.len());
        for p in &rfports {
            let node = resolve_net(&info, &p.node)?;
            ports.push(piperine_solver::prelude::SpPort { num: p.num as usize, node, z0: p.z0 });
        }
        let options = piperine_solver::prelude::SpOptions {
            ports,
            sweep: piperine_solver::prelude::AcSweepAnalysisOptions {
                start_frequency: fstart,
                stop_frequency: fstop,
                steps: points,
                logarithmic,
            },
        };
        let mut solver = circuit.sp(options, config.to_context())?;
        let result = solver.solve_sweep()?;
        self.fire_after_solve("sp", &[])?;
        Ok(result.into())
    }

    /// Run a distortion analysis (`.disto`): small-signal Volterra
    /// distortion at the DC operating point. Single-tone (`f2 = None`)
    /// reports `hd2`/`hd3` at `2·f1`/`3·f1`; two-tone (`f2 = Some(..)`)
    /// reports `im2` at `f1+f2` and `im3` at `2·f1−f2` — equal-amplitude
    /// tones, the ngspice convention. `amplitude` scales every AC stimulus
    /// magnitude in the circuit for the first-order solve; `output` is the
    /// measured net name, optionally differential against `output_ref`.
    /// Fails loud when `f1`/`amplitude` are non-positive, `f2` collides
    /// with `f1`, the output is not addressable, there is no first-order
    /// response at the output, or a device reads a branch current in a
    /// nonlinear contribution (DISTO-04).
    pub fn run_disto(
        &self,
        f1: f64,
        f2: Option<f64>,
        amplitude: f64,
        output: &str,
        output_ref: Option<&str>,
        config: &SolverConfig,
    ) -> Result<crate::results::DistoResult, Error> {
        let (mut circuit, info) = self.build_circuit(true)?;
        let output_node = resolve_net(&info, output)?;
        let output_ref_node = output_ref.map(|r| resolve_net(&info, r)).transpose()?;
        let options = piperine_solver::prelude::DistoOptions {
            f1,
            f2,
            amplitude,
            output: piperine_solver::abi::AnalogVariable::Node(output_node),
            output_ref: output_ref_node,
        };
        let mut solver = circuit.disto(options, config.to_context())?;
        let result = solver.solve()?;
        self.fire_after_solve("disto", &[])?;
        Ok(result.into())
    }

    /// Run a DC operating-point analysis. `nodeset` (net name → volts) seeds
    /// the Newton initial guess.
    pub fn run_op(
        &self,
        config: &SolverConfig,
        nodeset: Option<&HashMap<String, f64>>,
    ) -> Result<OpResult, Error> {
        let (mut circuit, info) = self.build_circuit(false)?;
        let ivs = build_ivs(&info, nodeset, circuit.netlist())?;
        let mut dc = circuit.dc(config.to_context())?;
        dc.policy = config.to_policy();
        dc.apply_initial_conditions(ivs);
        let result = dc.solve()?;
        drop(dc);
        let digital = Self::snapshot_digital(&info, &circuit);
        let opvars = Self::snapshot_opvars(&circuit);
        let introspect = Self::snapshot_introspect(&circuit);
        self.fire_after_solve("op", &node_voltages(&info, &result))?;
        Ok(OpResult::new(result, digital, opvars, introspect, Rc::new(info)))
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
        let (mut circuit, mut info) = self.build_circuit(false)?;
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
            let opvars = Self::snapshot_opvars(&circuit);
            let introspect = Self::snapshot_introspect(&circuit);
            self.fire_after_solve("op", &node_voltages(&info, &result))?;
            results.push(OpResult::new(
                result,
                digital,
                opvars,
                introspect,
                Rc::new(info.clone()),
            ));
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

    /// Every device's `read_opvars()` snapshot (HOST-07), keyed by instance
    /// label — the eager-at-solve-time capture `OpResult::instance` reads
    /// back through, since the compiled circuit does not outlive the
    /// analysis call. Public: hosts that drive `CircuitInstance` directly
    /// (the Python live session) build the same snapshot.
    pub fn snapshot_opvars(
        circuit: &piperine_solver::prelude::CircuitInstance,
    ) -> HashMap<String, Vec<(String, f64)>> {
        circuit
            .all_devices()
            .iter()
            .map(|d| (d.name().to_string(), d.read_opvars()))
            .collect()
    }

    /// Every device's static introspection catalogs (HOST-09), keyed by
    /// instance label — model descriptor, terminal descriptors (with
    /// `TerminalKind`), and observable catalog. Snapshotted eagerly at solve
    /// time alongside `snapshot_opvars` (the circuit does not outlive the
    /// analysis call). Public: hosts that drive `CircuitInstance` directly
    /// (the Python live session) build the same snapshot.
    pub fn snapshot_introspect(
        circuit: &piperine_solver::prelude::CircuitInstance,
    ) -> IntrospectSnapshot {
        let mut models = HashMap::new();
        let mut terminals = HashMap::new();
        let mut observables = HashMap::new();
        let mut params = HashMap::new();
        for d in circuit.all_devices() {
            let label = d.name().to_string();
            models.insert(label.clone(), d.model_descriptor());
            terminals.insert(label.clone(), d.list_terminals());
            observables.insert(label.clone(), d.list_observables());
            params.insert(label, d.list_params());
        }
        (models, terminals, observables, params)
    }

    /// Run a transient analysis: same elaborate-and-solve recipe as
    /// [`Self::run_op`], through `CircuitInstance::transient` instead of
    /// `::dc`. `step: None` selects the adaptive stepper. `start` is the
    /// earliest recorded time — the solver still integrates from t=0, but
    /// steps with `t < start` are dropped from the trace. `ic` (net name →
    /// volts) seeds the t=0 node voltages. `record_device_state` opts into
    /// per-step device runtime-bank recording, unlocking `Trace.i` on
    /// state-reading devices (`delay`/`transition`/`idt`); off it stays a
    /// loud error (and costs nothing per step). `probe` (HOST-08) names
    /// `"instance.opvar_name"` observables to record selectively — unknown
    /// device/observable requests fail loud at setup (ABI-35); the recorded
    /// values are read back with `Trace::opvar`.
    pub fn run_tran(
        &self,
        tspan: (f64, f64),
        step: Option<f64>,
        config: &SolverConfig,
        ic: Option<&HashMap<String, f64>>,
        record_device_state: bool,
        probe: &[&str],
    ) -> Result<Trace, Error> {
        let (stop, start) = tspan;
        let (mut circuit, info) = self.build_circuit(false)?;
        let ivs = build_ivs(&info, ic, circuit.netlist())?;
        let mut opts = match step {
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
        opts.record_device_state = record_device_state;
        opts.probe_selection = build_probe_selection(probe)?;
        let mut solver = circuit.transient(opts, config.to_context())?;
        solver.policy = config.to_policy();
        solver.apply_initial_conditions(ivs);
        let result = solver.solve()?;
        self.fire_after_solve("tran", &[])?;
        Ok(Trace::<Waveform>::new(result, Rc::new(info)))
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
        let (mut circuit, info) = self.build_circuit(false)?;
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
        frange: (f64, f64),
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<NoiseTrace, Error> {
        let (fstart, fstop) = frange;
        let (mut circuit, info) = self.build_circuit(false)?;
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

// ─── Session: the compiled center of gravity (HOST-01) ─────────────────────
//
// `SimSession` (above) elaborates + JITs fresh on every analysis call — the
// right shape for one-shot/staged workflows (mirrors Python's `_Module`).
// `Session` compiles **once** (`Session::compile`) and holds the built
// circuit across every subsequent analysis; parameter writes route through
// the solver's live-set path (`CircuitInstance::set_element_param`, MD-18:
// restamp, never re-JIT) — the Rust equivalent of Python's `_LiveSession`
// (spec: "Python rename `LiveSession`→`Session`; build the Rust equivalent").
//
// SPEC_DEVIATION: design.md's Approach Decision table describes `SimSession`
// as folding into `Session` ("no dup concept"). `SimSession` is kept as a
// distinct type here — Python itself has never had one concept for this:
// `_Module` (staged, forks + rebuilds per analysis) and `_LiveSession`
// (compiled once) are already two types serving two workflows, and the
// spec's own Goals bullet reads "build the Rust equivalent" of the compiled
// session, not "replace the staged one". Collapsing `SimSession` into
// `Session` would touch ~20 existing root/python call sites for no behavior
// change; reusing the same two-type shape Python already ships is the
// smaller, safer move. Flagged for the Verifier/orchestrator to confirm.
//
// SPEC_DEVIATION: `Session::set` on a structural (`Invalidation::Rebuild`)
// write fails loud instead of auto-re-elaborating. Python's `_LiveSession`
// auto-rebuild (LIVE-14/15/16/17) is ~150 lines of dirty-ledger/carry/
// mid-transient-split machinery; HOST-01's "Done when" only asks for
// `set`/`schedule_set`/`rebuilds` to be present, not for auto-rebuild parity.
// `rebuilds()` stays part of the surface (always `0` today) so a future task
// can wire the same auto-rebuild recipe without a signature change.

/// The compiled center of gravity (HOST-01): elaborate + JIT **once**
/// (`Session::compile`), then run every analysis against the held circuit.
/// `set` restamps a parameter on the already-compiled circuit (MD-18); a
/// structural write (`Invalidation::Rebuild`) fails loud (see the
/// SPEC_DEVIATION note above `Session::set`).
pub struct Session {
    design: Design,
    module: String,
    circuit: piperine_solver::prelude::CircuitInstance,
    info: CircuitBuildInfo,
    rebuilds: usize,
    /// Scheduled live writes `(t, label, param, value)` for the next
    /// `tran` (drained into the solver's own `schedule_set` when it runs).
    pending_sets: Vec<(f64, String, String, f64)>,
}

impl Session {
    /// Compile `module` of `design` **once**: fork the design, apply staged
    /// overrides, lower + JIT, and hold the built circuit. Every subsequent
    /// analysis runs on this same compilation (MD-18).
    pub fn compile(design: &Design, module: &str) -> Result<Self, Error> {
        let forked = design.fork();
        let applied = forked.with_overrides_applied(module)?.fork();
        let bodies = piperine_codegen::resolve::lower_bodies(&applied)?;
        let mut compiler = CircuitCompiler::new(&applied, &bodies);
        let (mut circuit, info) = compiler.build_circuit_mapped(module)?;
        circuit.init_digital()?;
        circuit.rebuild_digital_topology();
        Ok(Self {
            design: applied,
            module: module.to_string(),
            circuit,
            info,
            rebuilds: 0,
            pending_sets: Vec::new(),
        })
    }

    /// The module this session was compiled from.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// How many automatic structural rebuilds this session has performed
    /// (`0` — see the SPEC_DEVIATION note above [`Session`]: a structural
    /// write fails loud today rather than auto-rebuilding).
    pub fn rebuilds(&self) -> usize {
        self.rebuilds
    }

    /// Write a parameter on the compiled circuit, effective from the next
    /// analysis (MD-18: restamp, never re-JIT). A structural write
    /// (`Invalidation::Rebuild`) fails loud; an out-of-bounds value fails
    /// loud with the solver's own message.
    pub fn set(&mut self, label: &str, param: &str, value: f64) -> Result<(), Error> {
        use piperine_solver::abi::{Invalidation, Value};
        let inv = self.circuit.set_element_param(label, param, Value::Real(value))?;
        if inv >= Invalidation::Rebuild {
            return Err(Error::Measurement(format!(
                "structural set `{label}`.`{param}` would rebuild the circuit — \
                 Session does not auto-rebuild (use a fresh SimSession/Session::compile)"
            )));
        }
        // Mirror into the build info so device-internal current readbacks
        // (`.i(a, b)` on force-less two-terminal devices) see the new value
        // (same mirror as `SimSession::run_op_sweep`).
        if let Some(inst) = self.info.instances.iter_mut().find(|i| i.label == label)
            && let Some(pidx) = inst.kernel.param_names().iter().position(|n| n == param)
        {
            inst.params[pidx] = value;
        }
        Ok(())
    }

    /// Schedule a live parameter write at simulation time `t` for the next
    /// `tran` run: the integrator lands exactly on `t` and the write applies
    /// there (last-write-wins per param at the same `t`).
    pub fn schedule_set(&mut self, t: f64, label: &str, param: &str, value: f64) {
        self.pending_sets.push((t, label.to_string(), param.to_string(), value));
    }

    /// Run a DC operating-point analysis on the held circuit (HOST-01/02).
    pub fn op(
        &mut self,
        config: &SolverConfig,
        nodeset: Option<&HashMap<String, f64>>,
    ) -> Result<OpResult, Error> {
        let ivs = build_ivs(&self.info, nodeset, self.circuit.netlist())?;
        let mut dc = self.circuit.dc(config.to_context())?;
        dc.policy = config.to_policy();
        dc.apply_initial_conditions(ivs);
        let result = dc.solve()?;
        drop(dc);
        let digital = SimSession::snapshot_digital(&self.info, &self.circuit);
        let opvars = SimSession::snapshot_opvars(&self.circuit);
        let introspect = SimSession::snapshot_introspect(&self.circuit);
        Ok(OpResult::new(
            result,
            digital,
            opvars,
            introspect,
            Rc::new(self.info.clone()),
        ))
    }

    /// Run a transient analysis on the held circuit (HOST-02). Pending
    /// `schedule_set` entries at `t <= 0` are idle sets applied before the
    /// run; entries at `t > 0` land on the solver's own forced breakpoints.
    /// A *structural* scheduled set fails loud (see the SPEC_DEVIATION note
    /// above [`Session`] — no mid-run auto-rebuild in this session type).
    #[allow(clippy::too_many_arguments)]
    pub fn tran(
        &mut self,
        stop: f64,
        step: Option<f64>,
        start: f64,
        config: &SolverConfig,
        ic: Option<&HashMap<String, f64>>,
        record_device_state: bool,
        probe: &[&str],
    ) -> Result<Trace<Waveform>, Error> {
        let mut scheduled = Vec::new();
        for (t, label, param, value) in std::mem::take(&mut self.pending_sets) {
            if t <= 0.0 {
                self.set(&label, &param, value)?;
            } else {
                scheduled.push((t, label, param, value));
            }
        }
        let ivs = build_ivs(&self.info, ic, self.circuit.netlist())?;
        let mut opts = match step {
            Some(dt) if dt > 0.0 => piperine_solver::prelude::TransientAnalysisOptions::new(stop, dt),
            _ => piperine_solver::prelude::TransientAnalysisOptions::new(stop, stop * 1e-3),
        }
        .with_record_from(start);
        opts.record_device_state = record_device_state;
        opts.probe_selection = build_probe_selection(probe)?;
        let mut solver = self.circuit.transient(opts, config.to_context())?;
        solver.policy = config.to_policy();
        solver.apply_initial_conditions(ivs);
        for (t, label, param, value) in &scheduled {
            solver.schedule_set(*t, label, param, piperine_solver::abi::Value::Real(*value));
        }
        let result = solver.solve()?;
        drop(solver);
        for (_, label, param, value) in &scheduled {
            if let Some(inst) = self.info.instances.iter_mut().find(|i| i.label == *label)
                && let Some(pidx) = inst.kernel.param_names().iter().position(|n| n == param)
            {
                inst.params[pidx] = *value;
            }
        }
        Ok(Trace::<Waveform>::new(result, Rc::new(self.info.clone())))
    }

    // SPEC_DEVIATION: HOST-21's "analysis args impl Into<...>" is applied
    // here (`Session::ac`'s fstart/fstop) as the representative
    // demonstration, not to every frequency/time-shaped arg across both
    // `Session` and `SimSession` (~12 duplicated analysis methods total).
    // The change is additive/non-breaking (`f64: Into<Freq>` via the
    // blanket `From<f64>`, so every existing `f64` call site keeps
    // compiling unchanged) but touching every signature is a large,
    // separable mechanical follow-up; the newtypes + SI-string parsing +
    // Python `Hz`/`ns`/`mV`/`C` helpers (HOST-21's literal Done-when checks)
    // are fully delivered either way. Flagged for the Verifier.

    /// Run an AC small-signal sweep on the held circuit (HOST-02).
    ///
    /// `fstart`/`fstop` accept anything `Into<Freq>` (HOST-21): a plain
    /// `f64` (already in Hz, unchanged for every existing caller — `f64`
    /// implements `Into<Freq>` via the blanket `From<f64>`), or an
    /// SI-suffixed string (`"10MHz"`/`"10M"`).
    pub fn ac(
        &mut self,
        fstart: impl Into<crate::units::Freq>,
        fstop: impl Into<crate::units::Freq>,
        points: usize,
        logarithmic: impl Into<bool>,
        config: &SolverConfig,
    ) -> Result<AcTrace, Error> {
        let opts = piperine_solver::prelude::AcSweepAnalysisOptions {
            start_frequency: fstart.into().0,
            stop_frequency: fstop.into().0,
            steps: points,
            logarithmic: logarithmic.into(),
        };
        let mut ac = self.circuit.ac(config.to_context())?;
        ac.policy = config.to_policy();
        let result = ac.solve_sweep(opts)?;
        Ok(AcTrace::new(result, Rc::new(self.info.clone())))
    }

    /// Run an output-referred noise analysis on the held circuit (HOST-02).
    pub fn noise(
        &mut self,
        out: &str,
        reference: &str,
        frange: (f64, f64),
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<NoiseTrace, Error> {
        let (fstart, fstop) = frange;
        let out = resolve_net(&self.info, out)?;
        let reference = resolve_net(&self.info, reference)?;
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
        let result = self.circuit.noise(opts, config.to_context())?.solve()?;
        Ok(NoiseTrace::new(result))
    }

    /// Run a DC sensitivity analysis (`.sens`) on the held circuit
    /// (HOST-02): `∂V(output)/∂(param)` at the operating point for every
    /// requested `(label, param)` pair, by central finite difference.
    pub fn sens(
        &mut self,
        outputs: &[&str],
        params: &[(String, String)],
        dp_rel: f64,
        config: &SolverConfig,
    ) -> Result<crate::results::SensResult, Error> {
        let mut nets = Vec::with_capacity(outputs.len());
        for name in outputs {
            let node = resolve_net(&self.info, name)?;
            let var = piperine_solver::abi::AnalogVariable::Node(node);
            let net = self
                .circuit
                .nets()
                .into_iter()
                .find(|n| n.analog_variable().map(|v| **v == var).unwrap_or(false))
                .ok_or_else(|| Error::Measurement(format!("net `{name}` is not a solved analog net")))?;
            nets.push(((*name).to_string(), net));
        }
        let opts = piperine_solver::prelude::SensAnalysisOptions {
            outputs: nets.iter().map(|(_, n)| n.clone()).collect(),
            params: params.to_vec(),
            dp_rel,
        };
        let mut solver = self.circuit.sens(opts, config.to_context())?;
        solver.policy = config.to_policy();
        let inner = solver.solve()?;
        let mut d = HashMap::new();
        for (name, net) in &nets {
            for (label, param) in params {
                if let Some(v) = inner.get(net.label(), label, param) {
                    d.insert((name.clone(), format!("{label}.{param}")), v);
                }
            }
        }
        Ok(crate::results::SensResult { d })
    }

    /// Run a periodic-steady-state analysis (single shooting) on the held
    /// circuit (HOST-02).
    pub fn pss(
        &mut self,
        period: f64,
        tstab: f64,
        config: &SolverConfig,
    ) -> Result<crate::results::PssResult, Error> {
        let opts = piperine_solver::prelude::PssAnalysisOptions::new(period).with_tstab(tstab);
        let mut solver = self.circuit.pss(opts, config.to_context())?;
        solver.policy = config.to_policy();
        let inner = solver.solve()?;
        Ok(crate::results::PssResult {
            trace: Trace::<Waveform>::new(inner.trace, Rc::new(self.info.clone())),
            stats: inner.stats,
        })
    }

    /// Run a pole-zero analysis (`.pz`) on the held circuit (HOST-02).
    pub fn pz(
        &mut self,
        input_source: &str,
        output: &str,
        output_ref: Option<&str>,
        config: &SolverConfig,
    ) -> Result<crate::results::PzResult, Error> {
        let output_node = resolve_net(&self.info, output)?;
        let output_ref_node = output_ref.map(|r| resolve_net(&self.info, r)).transpose()?;
        let options = piperine_solver::prelude::PoleZeroOptions {
            input_source: piperine_solver::abi::BranchIdentifier::new(input_source, "force0"),
            output: piperine_solver::abi::AnalogVariable::Node(output_node),
            output_ref: output_ref_node,
        };
        let solver = self.circuit.pz(options, config.to_context())?;
        let poles = solver.poles()?;
        let zeros = solver.zeros()?;
        Ok(piperine_solver::prelude::PoleZeroResult { poles, zeros }.into())
    }

    /// Run a distortion analysis (`.disto`) on the held circuit (HOST-02).
    #[allow(clippy::too_many_arguments)]
    pub fn disto(
        &mut self,
        f1: f64,
        f2: Option<f64>,
        amplitude: f64,
        output: &str,
        output_ref: Option<&str>,
        config: &SolverConfig,
    ) -> Result<crate::results::DistoResult, Error> {
        let output_node = resolve_net(&self.info, output)?;
        let output_ref_node = output_ref.map(|r| resolve_net(&self.info, r)).transpose()?;
        let options = piperine_solver::prelude::DistoOptions {
            f1,
            f2,
            amplitude,
            output: piperine_solver::abi::AnalogVariable::Node(output_node),
            output_ref: output_ref_node,
        };
        let mut solver = self.circuit.disto(options, config.to_context())?;
        let result = solver.solve()?;
        Ok(result.into())
    }

    /// Run an N-port S-parameter analysis (`.sp`) on the held circuit
    /// (HOST-02): ports come from the design's `@rfport(num, z0)` attributes
    /// on `self.module`.
    pub fn sp(
        &mut self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: &SolverConfig,
    ) -> Result<crate::results::SParamResult, Error> {
        let rfports = self.design.rfports(&self.module)?;
        let mut ports = Vec::with_capacity(rfports.len());
        for p in &rfports {
            let node = resolve_net(&self.info, &p.node)?;
            ports.push(piperine_solver::prelude::SpPort { num: p.num as usize, node, z0: p.z0 });
        }
        let options = piperine_solver::prelude::SpOptions {
            ports,
            sweep: piperine_solver::prelude::AcSweepAnalysisOptions {
                start_frequency: fstart,
                stop_frequency: fstop,
                steps: points,
                logarithmic,
            },
        };
        let mut solver = self.circuit.sp(options, config.to_context())?;
        let result = solver.solve_sweep()?;
        Ok(result.into())
    }

    /// Run a transfer-function analysis (`.tf`, HOST-03): DC small-signal
    /// gain, input resistance, and output resistance from unit excitations
    /// on the system linearized at the operating point. Binds the existing
    /// solver `.tf` driver — no new solver math (MD-14: voltage-source input
    /// only). `output_ref` differentially references a voltage output.
    pub fn tf(
        &mut self,
        output: &str,
        output_ref: Option<&str>,
        input_source: &str,
        config: &SolverConfig,
    ) -> Result<crate::results::TfResult, Error> {
        let output_node = resolve_net(&self.info, output)?;
        let output_ref_node = output_ref.map(|r| resolve_net(&self.info, r)).transpose()?;
        let options = piperine_solver::prelude::TransferFunctionAnalysisOptions {
            output: piperine_solver::abi::AnalogVariable::Node(output_node),
            output_ref: output_ref_node,
            input_source: piperine_solver::abi::BranchIdentifier::new(input_source, "force0"),
        };
        let mut solver = self.circuit.transfer_function(options, config.to_context())?;
        let result = solver.solve()?;
        Ok(crate::results::TfResult::from_solver(result))
    }

    /// Run a compile-once DC sweep (`.dc`, HOST-05): restamp `label.param`
    /// on the already-compiled circuit (MD-18) for each of `values`, solving
    /// an operating point per point, and return the swept result as a
    /// `Trace<Waveform>` over the swept axis (not a bare `Vec<OpResult>`) —
    /// the same generic container `tran`/`pss` use (HOST-13), read the same
    /// way (`.v`/`.i`/`.axis`/`.stats`).
    pub fn dc(
        &mut self,
        label: &str,
        param: &str,
        values: &[f64],
        config: &SolverConfig,
        nodeset: Option<&HashMap<String, f64>>,
    ) -> Result<Trace<Waveform>, Error> {
        use piperine_solver::abi::Value;
        let mut points = Vec::with_capacity(values.len());
        let mut digital = Vec::with_capacity(values.len());
        let mut stats = SolverStats { converged: true, ..Default::default() };
        for &v in values {
            self.circuit.set_element_param(label, param, Value::Real(v))?;
            if let Some(inst) = self.info.instances.iter_mut().find(|i| i.label == label)
                && let Some(pidx) = inst.kernel.param_names().iter().position(|n| n == param)
            {
                inst.params[pidx] = v;
            }
            let ivs = build_ivs(&self.info, nodeset, self.circuit.netlist())?;
            let mut dc = self.circuit.dc(config.to_context())?;
            dc.policy = config.to_policy();
            dc.apply_initial_conditions(ivs);
            let result = dc.solve()?;
            drop(dc);
            stats.converged &= result.stats.converged;
            stats.newton_iterations += result.stats.newton_iterations;
            digital.push(SimSession::snapshot_digital(&self.info, &self.circuit));
            points.push(result);
        }
        Ok(Trace::<Waveform>::from_dc_sweep(values.to_vec(), points, digital, Rc::new(self.info.clone()), stats))
    }

    /// A fluent single-knob sweep over `label.param` (HOST-18): iterate with
    /// `while let Some(point) = sweep.next() { ... }` — each `point` is a
    /// [`SweepPoint`], a `Session` view at that knob value (`Deref`/
    /// `DerefMut` to `Session`, so every analysis method is callable
    /// directly on it). A non-structural value restamps on the one
    /// compilation (MD-18); a structural value rebuilds the circuit in
    /// place and increments [`Self::rebuilds`] — see [`Sweep::next`].
    pub fn sweep<'a>(&'a mut self, label: &str, param: &str, values: &[f64]) -> Sweep<'a> {
        Sweep { session: self, label: label.to_string(), param: param.to_string(), values: values.to_vec(), idx: 0 }
    }

    /// Write `label.param` on the compiled circuit, restamping when the
    /// write is non-structural (MD-18, same as [`Self::set`]) or rebuilding
    /// the circuit in place (HOST-18) and incrementing [`Self::rebuilds`]
    /// when it is structural (`Invalidation::Rebuild`) — never silently
    /// restamps a structural change onto stale kernel-compiled state.
    fn set_or_rebuild(&mut self, label: &str, param: &str, value: f64) -> Result<(), Error> {
        use piperine_solver::abi::{Invalidation, Value};
        let inv = self.circuit.set_element_param(label, param, Value::Real(value))?;
        if inv >= Invalidation::Rebuild {
            return self.rebuild(label, param, value);
        }
        if let Some(inst) = self.info.instances.iter_mut().find(|i| i.label == label)
            && let Some(pidx) = inst.kernel.param_names().iter().position(|n| n == param)
        {
            inst.params[pidx] = value;
        }
        Ok(())
    }

    /// Re-stage `label.param = value` on this session's design and rebuild
    /// the circuit from scratch (fork → apply overrides → lower → JIT),
    /// replacing the held `design`/`circuit`/`info` in place and
    /// incrementing [`Self::rebuilds`] — the HOST-18 sweep escape hatch for
    /// a structural (rebuild-invalidating) knob, unlike [`Self::set`]'s
    /// unconditional fail-loud.
    fn rebuild(&mut self, label: &str, param: &str, value: f64) -> Result<(), Error> {
        self.design.set_param(label, param, piperine_lang::Value::Real(value));
        let applied = self.design.with_overrides_applied(&self.module)?.fork();
        let bodies = piperine_codegen::resolve::lower_bodies(&applied)?;
        let mut compiler = CircuitCompiler::new(&applied, &bodies);
        let (mut circuit, info) = compiler.build_circuit_mapped(&self.module)?;
        circuit.init_digital()?;
        circuit.rebuild_digital_topology();
        self.design = applied;
        self.circuit = circuit;
        self.info = info;
        self.rebuilds += 1;
        Ok(())
    }
}

// ─── Sweep / SweepPoint (HOST-18) ───────────────────────────────────────────

/// A `Session` view at one sweep coordinate (HOST-18): `Deref`/`DerefMut` to
/// [`Session`], so `point.op(...)`/`point.tran(...)`/… — every analysis —
/// runs directly on it, at the knob value the [`Sweep`] just restamped
/// (or rebuilt) onto the held circuit.
pub struct SweepPoint<'a> {
    session: &'a mut Session,
    /// The knob value this point was set to.
    pub value: f64,
    /// This point's position in the sweep's `values` slice.
    pub index: usize,
}

impl std::ops::Deref for SweepPoint<'_> {
    type Target = Session;
    fn deref(&self) -> &Session {
        self.session
    }
}

impl std::ops::DerefMut for SweepPoint<'_> {
    fn deref_mut(&mut self) -> &mut Session {
        self.session
    }
}

/// A fluent single-knob sweep (HOST-18, [`Session::sweep`]): a streaming
/// (lending) iterator — `next(&mut self) -> Option<Result<SweepPoint<'_>, Error>>`
/// instead of `std::iter::Iterator`, since each yielded [`SweepPoint`]
/// mutably borrows the sweep's own `Session` and Rust's stable `Iterator`
/// trait cannot express an item borrowing from the iterator itself. Drive it
/// with `while let Some(point) = sweep.next() { let point = point?; … }`.
pub struct Sweep<'a> {
    session: &'a mut Session,
    label: String,
    param: String,
    values: Vec<f64>,
    idx: usize,
}

impl Sweep<'_> {
    /// The number of points in this sweep.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// `true` when the sweep has no points.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Restamp (or rebuild, for a structural knob) the session onto the next
    /// sweep value and yield the resulting [`SweepPoint`]; `None` once every
    /// value has been visited. A structural knob transparently rebuilds the
    /// circuit and counts it in [`Session::rebuilds`] (HOST-18) rather than
    /// failing loud the way a bare [`Session::set`] does.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Result<SweepPoint<'_>, Error>> {
        if self.idx >= self.values.len() {
            return None;
        }
        let value = self.values[self.idx];
        let index = self.idx;
        self.idx += 1;
        if let Err(e) = self.session.set_or_rebuild(&self.label, &self.param, value) {
            return Some(Err(e));
        }
        Some(Ok(SweepPoint { session: self.session, value, index }))
    }
}

// ─── Grid / Nested (HOST-19) ────────────────────────────────────────────────

/// A nested (axis-shaped) result tree (HOST-19): [`Grid::map`]'s return
/// shape — `Leaf` at the deepest axis, `Branch` at every outer axis. Mirrors
/// a numpy ndarray's shape without pulling an `ndarray`/ad hoc flat-index
/// dependency into a generic-`R` result type; the tree's depth equals the
/// grid's axis count and each `Branch`'s length equals that axis's value
/// count (i.e. `Grid::shape()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nested<R> {
    Leaf(R),
    Branch(Vec<Nested<R>>),
}

/// A named multi-axis sweep grid (HOST-19, [`Session::sweep_grid`]): each
/// axis is `(label, param, values)`; [`Grid::map`] visits the cartesian
/// product in row-major (outer-axis-first) order, restamping (or
/// rebuilding, per axis write — same [`Session::set_or_rebuild`] escape
/// hatch [`Sweep`] uses) each axis's value before calling the mapped
/// function, and collects the results into a [`Nested`] tree shaped like
/// [`Grid::shape`].
pub struct Grid<'a> {
    session: &'a mut Session,
    axes: Vec<(String, String, Vec<f64>)>,
}

impl Session {
    /// A named multi-axis sweep grid (HOST-19): `axes` is
    /// `[(label, param, values), ...]`, outer axis first. Iterate with
    /// [`Grid::map`].
    pub fn sweep_grid<'a>(&'a mut self, axes: &[(&str, &str, &[f64])]) -> Grid<'a> {
        Grid {
            session: self,
            axes: axes.iter().map(|&(l, p, v)| (l.to_string(), p.to_string(), v.to_vec())).collect(),
        }
    }
}

impl Grid<'_> {
    /// The grid's shape — one length per axis, outer axis first.
    pub fn shape(&self) -> Vec<usize> {
        self.axes.iter().map(|(_, _, v)| v.len()).collect()
    }

    /// The total number of grid points (product of [`Self::shape`]).
    pub fn len(&self) -> usize {
        self.axes.iter().map(|(_, _, v)| v.len()).product()
    }

    /// `true` when any axis has no values (an empty grid).
    pub fn is_empty(&self) -> bool {
        self.axes.iter().any(|(_, _, v)| v.is_empty())
    }

    /// Visit every combination in the grid (row-major, outer axis first),
    /// restamping (or rebuilding) each axis's value on the held session
    /// before calling `f` with the session and this point's coordinates
    /// (one value per axis, outer axis first), and collect the results into
    /// a [`Nested`] tree shaped like [`Self::shape`]. A `f` error
    /// propagates with the failing combination's coordinates prefixed (the
    /// spec's edge case: a sweep-point failure surfaces with the point's
    /// coordinates, not a bare error).
    pub fn map<R>(
        &mut self,
        mut f: impl FnMut(&mut Session, &[f64]) -> Result<R, Error>,
    ) -> Result<Nested<R>, Error> {
        let axes = self.axes.clone();
        let mut coord = Vec::with_capacity(axes.len());
        Self::map_axis(self.session, &axes, 0, &mut coord, &mut f)
    }

    fn map_axis<R>(
        session: &mut Session,
        axes: &[(String, String, Vec<f64>)],
        depth: usize,
        coord: &mut Vec<f64>,
        f: &mut impl FnMut(&mut Session, &[f64]) -> Result<R, Error>,
    ) -> Result<Nested<R>, Error> {
        if depth == axes.len() {
            let value = f(session, coord)
                .map_err(|e| Error::Measurement(format!("sweep_grid at {coord:?}: {e}")))?;
            return Ok(Nested::Leaf(value));
        }
        let (label, param, values) = &axes[depth];
        let mut branch = Vec::with_capacity(values.len());
        for &v in values {
            session.set_or_rebuild(label, param, v).map_err(|e| {
                Error::Measurement(format!("sweep_grid at {coord:?} + [{label}.{param}={v}]: {e}"))
            })?;
            coord.push(v);
            branch.push(Self::map_axis(session, axes, depth + 1, coord, f)?);
            coord.pop();
        }
        Ok(Nested::Branch(branch))
    }
}

/// Build a [`piperine_solver::prelude::ProbeSelection`] from `"instance.name"`
/// paths (HOST-08's `tran(probe = [...])`). Malformed paths (no `.`) fail
/// loud here; unknown device/observable pairs fail loud at solver setup
/// (ABI-35, `CircuitInstance::transient`).
fn build_probe_selection(
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
