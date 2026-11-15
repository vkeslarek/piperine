//! DC adjoint sensitivity (dv-gradients T8, DVG-08/10): one transpose
//! solve of `Jᵀλ = ∂f/∂x` at the converged operating point, combined with
//! the per-parameter `∂F/∂p` from [`ParamStampCollector`], yields
//! `∂f/∂p = −λᵀ·∂F/∂p` for **every** requested parameter from that one
//! solve. This is the economic heart of the adjoint: `.sens`'s two
//! re-solves per parameter collapse to one extra linear solve for all of
//! them, regardless of count.
//!
//! Sibling of [`analyses/noise.rs:240`](crate::analyses::noise)'s complex
//! per-frequency adjoint — same idea, real-valued, a different code path.
//! The spec (D6) names this distinction deliberately so the optimistic
//! "reuse the noise adjoint" reading cannot survive.
//!
//! The driver is a DC analysis variant: it solves the operating point
//! through [`DcSolver`], re-stamps the converged Jacobian once, and asks
//! each element for `∂F/∂p` through the element ABI
//! ([`AnalogDevice::param_stamps`](crate::core::element::AnalogDevice::param_stamps)).
//! The solver never depends on codegen — `∂F/∂p` arrives through the ABI.

use std::collections::{HashMap, HashSet};

use crate::analyses::context::Context;
use crate::analyses::dc::{DcAnalysisState, DcSolver};
use crate::analyses::sensitivity::{ParamRequest, ParamStampCollector};
use piperine_analog_solver::tolerances::Policy;
use piperine_analog_solver::netlist::AnalogReference;
use crate::core::circuit::CircuitInstance;
use crate::core::net::Net;
use crate::core::result::Result;
use piperine_digital_solver::events::LogicValue;
use piperine_analog_solver::error::{Error, SolverDomain};
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use piperine_analog_solver::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::prelude::DcAnalysisResult;

use ndarray::Array1;

/// One DC adjoint request: a scalar output (solved node voltage or branch
/// current, optionally differential) and the parameters to differentiate
/// w.r.t. Mirrors
/// [`SensAnalysisOptions`](crate::analyses::sens::SensAnalysisOptions)'s
/// shape so a host can swap `.sens` for `.adjoint` without re-articulating
/// the request.
#[derive(Debug, Clone)]
pub struct AdjointOptions {
    pub output: Net,
    /// Optional reference for a differential read `output − output_ref`
    /// (ground-referenced when `None`). The transpose solve places `+1`
    /// at `output`'s row and `−1` at `output_ref`'s row in `∂f/∂x` —
    /// still one solve, regardless.
    pub output_ref: Option<Net>,
    pub params: Vec<(String, String)>,
    /// For a circuit of devices: difference `∂F/∂p` by re-evaluation instead of
    /// the declared partials (the oracle of the declared route).
    pub finite_difference_only: bool,
}

impl AdjointOptions {
    pub fn new(output: Net, params: Vec<(String, String)>) -> Self {
        Self { output, output_ref: None, params, finite_difference_only: false }
    }

    /// Differential output `output − output_ref` — the shape a margin on
    /// `V(a, b)` needs. One transpose solve still serves every parameter.
    pub fn new_diff(output: Net, output_ref: Net, params: Vec<(String, String)>) -> Self {
        Self { output, output_ref: Some(output_ref), params, finite_difference_only: false }
    }
}

/// The DC adjoint result: `∂(output)/∂(label.param)` per request, with the
/// solve-count proof and engine attribution.
#[derive(Debug, Clone)]
pub struct AdjointResult {
    /// `∂(output)/∂(label.param)` keyed by `(output label, "label.param")`,
    /// the same key shape [`SensResult`](crate::analyses::sens::SensResult)
    /// uses so a host can drop-in replace.
    pub d: HashMap<(String, String), f64>,
    /// How many transpose solves the driver ran (DVG-10). Exactly **one**
    /// regardless of parameter count — the whole point of the adjoint.
    pub transpose_solves: usize,
    /// Labels of elements that used the stamp-perturbation fallback
    /// (DVG-07) — propagated from [`ParamStampCollector::fallback_users`].
    pub fallback_users: HashSet<String>,
    /// Newton iterations the underlying DC solve cost — the floor the
    /// `transpose_solves = 1` count is added to. Reported so a host can
    /// quote the honest total `newton_iterations + transpose_solves`.
    pub newton_iterations: usize,
}

impl AdjointResult {
    /// `∂(output)/∂(label.param)`, if computed.
    pub fn get(&self, output: &str, label: &str, param: &str) -> Option<f64> {
        self.d
            .get(&(output.to_string(), format!("{label}.{param}")))
            .copied()
    }
}

/// The DC adjoint driver.
pub struct AdjointSolver<'a> {
    circuit: &'a mut CircuitInstance,
    options: AdjointOptions,
    context: Context,
    /// Convergence tunables applied to the inner DC solve (MD-04).
    pub policy: Policy,
}

impl<'a> AdjointSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: AdjointOptions,
        context: Context,
    ) -> Result<Self> {
        Ok(Self { circuit, options, context, policy: Policy::default() })
    }

    /// Solve `Jᵀλ = ∂f/∂x` once at the converged operating point and combine
    /// with each parameter's `∂F/∂p` (DVG-08). The transpose-solve count is
    /// exactly one for the whole parameter list — never one per parameter.
    pub fn solve(self) -> Result<AdjointResult> {
        // Validate the output is a solved analog net — the same gate
        // `.sens` applies. A digital net or pseudo net fails loud.
        if self.options.output.analog_variable().is_none() {
            return Err(Error::simple(
                SolverDomain::Sens,
                format!(
                    "adjoint output `{}` is not a solved analog net",
                    self.options.output.label()
                ),
            ));
        }

        // 1. Solve the operating point. The DC driver owns convergence,
        //    homotopy, and the mixed-signal settle — the adjoint reads the
        //    converged point and never re-solves it.
        let mut dc = DcSolver::new(self.circuit, self.context.clone())?;
        dc.policy = self.policy.clone();
        let dc_result = dc.solve()?;
        drop(dc);
        let newton_iterations = dc_result.stats.newton_iterations;

        // 2. Build the op vector indexed by `AnalogReference::idx`, the shape
        //    `load_dc` and `param_stamps` read. Ground rows stay 0.
        let op = op_vector(self.circuit, &dc_result);
        let size = self.circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        // 3. Seed the OP into each device's `last_volts` (dv-gradients B1
        //    note: a pure-analog DC solve does not call
        //    `accept_and_run_digital`, so the adjoint seeds the same way
        //    before reading `param_stamps` — the JIT kernel reads its
        //    cached terminal voltages off the accept-gated `last_volts`).
        if self.circuit.abi.is_none() {
            let _ = self.circuit.accept_and_run_digital(&op, 0.0);
        }

        // 4. Re-stamp the Jacobian once at the converged op. Only Matrix
        //    entries feed the transpose solve's LHS; the residual Rhs the
        //    devices also produced here is the DC source vector, not the
        //    adjoint's ∂f/∂x RHS (which is set explicitly below).
        let stamps = restamp_jacobian(self.circuit, &op, &self.context)?;
        let matrix_stamps: Vec<Stamp<AnalogReference, f64>> = stamps
            .into_iter()
            .filter(|s| matches!(s, Stamp::Matrix(..)))
            .collect();

        let symbolic = FaerSymbolicMatrix::new(size, matrix_stamps.clone())?;

        // 5. The output unknown's row index — where ∂f/∂x places its `+1`
        //    (and `−1` at the optional differential reference). Loud when
        //    the primary output is not a solved analog unknown; the
        //    optional reference is allowed to be ground (no MNA row),
        //    treated as "no `−1` contribution" — mathematically correct,
        //    since V(gnd) = 0 differentiates to zero.
        let out_idx =
            output_index(self.circuit, &self.options.output, &dc_result, &op, size)?;
        let out_ref_idx = match &self.options.output_ref {
            Some(r) => output_index(self.circuit, r, &dc_result, &op, size).ok(),
            None => None,
        };

        // 6. ONE transpose solve: Jᵀλ = ∂f/∂x, where ∂f/∂x = e_out for a
        //    ground-referenced output or e_out − e_ref for a differential.
        let mut system: FaerSparseLinearSystem<f64> = LinearSystem::new(size);
        system.apply_stamps(matrix_stamps);
        // The RHS is ∂f/∂x — overwrite anything apply_stamps wrote into
        // b_vec, since the device residual Rhs is not part of this system.
        for v in &mut system.b_vec {
            *v = 0.0;
        }
        system.b_vec[out_idx] = 1.0;
        if let Some(r) = out_ref_idx {
            system.b_vec[r] = -1.0;
        }
        let lambda = system.solve_transpose_with_backend(&symbolic)?;
        let transpose_solves = 1;

        // 7. For each requested parameter, collect ∂F/∂p and combine:
        //    ∂f/∂p = −λᵀ·∂F/∂p. The collector records the engine
        //    (analytic vs stamp-perturbation, DVG-07) and which elements
        //    used the fallback — propagated onto the result.
        let mut collector = ParamStampCollector::new(self.circuit, self.context.clone());
        if self.options.finite_difference_only {
            collector = collector.finite_difference_only();
        }
        let out_label = self.options.output.label().to_string();
        let mut d = HashMap::new();
        for (label, param) in &self.options.params {
            let req = ParamRequest { label: label.clone(), param: param.clone() };
            let deriv = collector.collect(&req, &op)?;
            let mut grad = 0.0;
            for s in &deriv.stamps {
                if let Stamp::Rhs(r, v) = s
                    && let Some(i) = r.idx()
                {
                    grad -= lambda[i] * v;
                }
            }
            d.insert((out_label.clone(), format!("{label}.{param}")), grad);
        }
        let fallback_users = collector.fallback_users.clone();

        Ok(AdjointResult {
            d,
            transpose_solves,
            fallback_users,
            newton_iterations,
        })
    }
}

/// Re-stamp the Jacobian at the converged operating point: build the
/// 1-row history buffer the devices read from `op`, call `load_dc` on each
/// device once, and concatenate the stamps. This is the same shape the
/// noise driver's `assemble_linearized` uses for AC, applied to the DC
/// residual — the matrix half is `∂F/∂x`, the adjoint's `J`.
fn restamp_jacobian(
    circuit: &mut CircuitInstance,
    op: &[f64],
    ctx: &Context,
) -> Result<Vec<Stamp<AnalogReference, f64>>> {
    if let Some(bound) = circuit.abi.as_mut() {
        let amb = piperine_analog_solver::device::Ambient {
            time: 0.0,
            freq: 0.0,
            is_static: true,
            gmin: ctx.tolerances.gmin,
            source_scale: 1.0,
        };
        return bound.analog.assemble_dc(op, amb);
    }
    let mut buf = CircularArrayBuffer2::new(1, op.len());
    let row = Array1::from_vec(op.to_vec());
    buf.push(&row.view());
    let digital: &[LogicValue] = &[];
    let dc_state = DcAnalysisState::new(&buf, digital, 1.0);
    // refresh cached voltages from the op before stamping (mirrors
    // `DcSystem::assemble`'s `update_all` step).
    circuit.update_all(&buf, ctx);
    let mut stamps = Vec::new();
    for dc in circuit.devices.iter_mut() {
        stamps.extend(dc.load_dc(&dc_state, ctx));
    }
    Ok(stamps)
}

/// Materialize the converged DC solution into the index-addressed vector
/// the transpose solve and `param_stamps` read. Mirrors the helper in
/// `tests/sensitivity_fallback.rs` — pulled in-crate so the driver and its
/// tests share one shape.
fn op_vector(circuit: &CircuitInstance, dc: &DcAnalysisResult) -> Vec<f64> {
    let pairs: Vec<(usize, f64)> = circuit
        .netlist()
        .all_references()
        .iter()
        .filter_map(|r| Some((r.idx()?, dc.get(r.variable().clone())?)))
        .collect();
    let n = pairs.iter().map(|(i, _)| i + 1).max().unwrap_or(0);
    let mut op = vec![0.0; n];
    for (i, v) in pairs {
        op[i] = v;
    }
    op
}

/// Resolve the output net to its row index in the Jacobian. A node output
/// is read directly from the op vector; a branch output reads the branch
/// unknown's index from the netlist. Loud when the net is not a solved
/// analog unknown at this size.
fn output_index(
    circuit: &CircuitInstance,
    output: &Net,
    dc: &DcAnalysisResult,
    op: &[f64],
    size: usize,
) -> Result<usize> {
    let Some(var) = output.analog_variable() else {
        return Err(Error::simple(
            SolverDomain::Sens,
            format!(
                "adjoint output `{}` is not a solved analog net",
                output.label()
            ),
        ));
    };
    // Prefer the netlist's index for the variable (covers nodes and
    // branches uniformly); fall back to scanning `op` for the value if the
    // variable is not in the netlist (defensive — the DC result covers
    // every solved unknown).
    let idx = circuit
        .netlist()
        .all_references()
        .iter()
        .find(|r| r.variable().as_ref() == var.as_ref())
        .and_then(|r| r.idx());
    if let Some(i) = idx
        && i < size
    {
        return Ok(i);
    }
    // The DC result must have produced a value for this variable — if it
    // did not, the requested output was not solved.
    dc.get(var.as_ref().clone())
        .ok_or_else(|| {
            Error::simple(
                SolverDomain::Sens,
                format!(
                    "adjoint output `{}` is not in the solved DC result",
                    output.label()
                ),
            )
        })?;
    // The variable has a value but no netlist index (e.g. a future
    // pseudo-unknown) — the transpose solve needs an index, so fail loud.
    let _ = op;
    Err(Error::simple(
        SolverDomain::Sens,
        format!(
            "adjoint output `{}` has no Jacobian row index — not a solved MNA unknown",
            output.label()
        ),
    ))
}
