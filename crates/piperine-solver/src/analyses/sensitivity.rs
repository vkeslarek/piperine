//! DC parameter-sensitivity stamp collection (dv-gradients DVG-04/06/07): the
//! `∂F/∂param` collector that, at the converged operating point, asks each
//! element for its parameter derivative — analytically where the element
//! declares [`HAS_SENSITIVITY`](crate::core::element::ElementCapabilities::HAS_SENSITIVITY)
//! (DVG-04/05), by stamp perturbation otherwise (DVG-06).
//!
//! The fallback differences two [`AnalogDevice::load_dc`] calls at the **same**
//! operating point (the parameter is perturbed, the voltages are not) — never a
//! re-solve — so the adjoint economy (one transpose solve for every parameter)
//! survives: only `∂F/∂p` becomes approximate, never the solve count.
//!
//! Every derivative carries the engine that produced it, and the collector
//! names each element that needed the fallback, by label (DVG-07) — never a
//! silent accuracy change. The DC adjoint driver (T8) wraps this collector with
//! the real transpose solve (T7).

use std::collections::{HashMap, HashSet};

use ndarray::Array1;

use crate::analyses::context::Context;
use crate::analyses::dc::DcAnalysisState;
use piperine_analog_solver::netlist::AnalogReference;
use crate::core::circuit::CircuitInstance;
use crate::core::element::ElementCapabilities;
use crate::core::introspect::{Invalidation, ParamError, Value};
use crate::core::result::Result;
use piperine_digital_solver::events::LogicValue;
use piperine_analog_solver::error::{Error, SolverDomain};
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::linear::Stamp;

/// One parameter-derivative request: differentiate element `label`'s residual
/// w.r.t. its parameter `param`, at the converged operating point.
#[derive(Debug, Clone)]
pub struct ParamRequest {
    pub label: String,
    pub param: String,
}

/// How a parameter derivative was computed (DVG-07 — always reported).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitivityEngine {
    /// Analytic `∂F/∂p` kernel — the element declared `HAS_SENSITIVITY`.
    Analytic,
    /// Stamp perturbation at the operating point — the element lacks the bit.
    StampPerturbation,
}

/// The collected `∂F/∂param` for one request: residual-shaped stamps plus the
/// engine that produced them. `Stamp::Rhs(ref, ∂F_ref/∂param)` per MNA row.
#[derive(Debug, Clone)]
pub struct ParamDerivative {
    pub label: String,
    pub param: String,
    pub stamps: Vec<Stamp<AnalogReference, f64>>,
    pub engine: SensitivityEngine,
}

/// Relative finite-difference step for the stamp-perturbation fallback
/// (absolute fallback when the parameter value is ~0). Mirrors `.sens`.
const DP_REL: f64 = 1e-6;

fn step(value: f64) -> f64 {
    let base = value.abs().max(1e-12);
    (DP_REL * base).max(1e-12)
}

/// Collects `∂F/∂param` per request, recording which elements used the
/// stamp-perturbation fallback (DVG-07). One collector per operating point;
/// the converged node voltages are passed to [`ParamStampCollector::collect`].
pub struct ParamStampCollector<'a> {
    circuit: &'a mut CircuitInstance,
    context: Context,
    /// Labels of elements that used the stamp-perturbation fallback.
    pub fallback_users: HashSet<String>,
}

impl<'a> ParamStampCollector<'a> {
    pub fn new(circuit: &'a mut CircuitInstance, context: Context) -> Self {
        Self { circuit, context, fallback_users: HashSet::new() }
    }

    /// Collect `∂F/∂param` for `req` at the converged operating point `op`
    /// (node voltages indexed by [`AnalogReference::idx`]). No re-solve: the
    /// analytic path reads the cached kernel; the fallback re-stamps at the
    /// same voltages. Fails loud (reusing `.sens`'s rule) for an unknown
    /// element/parameter, a rebuild-class parameter, or a non-Real parameter.
    pub fn collect(&mut self, req: &ParamRequest, op: &[f64]) -> Result<ParamDerivative> {
        // Admissibility + metadata first — a short mutable borrow of the one
        // device, released before the netlist read (fallback) re-borrows.
        let (param_idx, value, has_kernel) = {
            let device = self.find_device(&req.label)?;
            let desc = device
                .list_params()
                .into_iter()
                .find(|p| p.name == req.param)
                .ok_or_else(|| {
                    let names: Vec<String> =
                        device.list_params().into_iter().map(|p| p.name).collect();
                    Error::simple(
                        SolverDomain::Sens,
                        format!(
                            "`{}` declares no parameter `{}`; available: {}",
                            req.label,
                            req.param,
                            if names.is_empty() { "(none)".into() } else { names.join(", ") }
                        ),
                    )
                })?;
            let param_idx = device
                .list_params()
                .iter()
                .position(|p| p.name == req.param)
                .expect("just found the descriptor");
            // Reuse `.sens`'s admissibility rule (one rule, not a copy):
            // restamp-class + Real. Rebuild or discrete → fail loud.
            if desc.invalidation == Invalidation::Rebuild {
                return Err(Error::simple(
                    SolverDomain::Sens,
                    format!(
                        "`{}.{}` invalidates the compiled circuit (Rebuild) — \
                         sensitivities are only defined for restampable parameters",
                        req.label, req.param
                    ),
                ));
            }
            let value = match device.get_param(&req.param) {
                Some(Value::Real(v)) => v,
                Some(_) => {
                    return Err(Error::simple(
                        SolverDomain::Sens,
                        format!(
                            "`{}.{}` is not Real-typed — only Real parameters are differentiable",
                            req.label, req.param
                        ),
                    ));
                }
                None => {
                    return Err(Error::simple(
                        SolverDomain::Sens,
                        format!("`{}.{}` has no value", req.label, req.param),
                    ));
                }
            };
            let has_kernel = device
                .capabilities()
                .contains(ElementCapabilities::HAS_SENSITIVITY);
            (param_idx, value, has_kernel)
        };

        if has_kernel {
            // Analytic path: the JIT kernel, into a caller-owned sink.
            let mut sink: Vec<Stamp<AnalogReference, f64>> = Vec::new();
            let device = self.find_device(&req.label)?;
            device.param_stamps(param_idx, &mut sink);
            // A parameter the kernel excludes (e.g. presence-queried) yields no
            // stamps — a documented conservative gap, reported as analytic-zero
            // rather than silently swapping in a fallback.
            return Ok(ParamDerivative {
                label: req.label.clone(),
                param: req.param.clone(),
                stamps: sink,
                engine: SensitivityEngine::Analytic,
            });
        }

        // Fallback path: stamp perturbation at the operating point. Record the
        // element label (DVG-07) before borrowing the device.
        self.fallback_users.insert(req.label.clone());
        // Reference map (node index → AnalogReference) for emitting stamps; built
        // from the netlist before the mutable device borrow so the borrows don't
        // overlap. Context is cloned for the same reason (disjoint from circuit).
        let ref_map = index_refs(self.circuit.netlist().all_references());
        let ctx = self.context.clone();
        let device = self.find_device(&req.label)?;
        let stamps = perturb(device, &req.param, value, op, &ref_map, &ctx)?;
        Ok(ParamDerivative {
            label: req.label.clone(),
            param: req.param.clone(),
            stamps,
            engine: SensitivityEngine::StampPerturbation,
        })
    }

    fn find_device(&mut self, label: &str) -> Result<&mut dyn crate::core::element::Element> {
        let device = self
            .circuit
            .devices
            .iter_mut()
            .find(|d| d.name() == label)
            .ok_or_else(|| Error::simple(SolverDomain::Sens, format!("no element labeled `{label}`")))?;
        Ok(device.as_mut())
    }
}

/// Build `node index → AnalogReference` from the netlist's references, so the
/// fallback can emit `Stamp::Rhs` for a computed row index.
fn index_refs(refs: Vec<&AnalogReference>) -> HashMap<usize, AnalogReference> {
    let mut map = HashMap::new();
    for r in refs {
        if let Some(i) = r.idx() {
            map.entry(i).or_insert_with(|| r.clone());
        }
    }
    map
}

/// Forward-difference `∂F/∂param` for one non-`HAS_SENSITIVITY` element by
/// re-stamping at the same operating point (DVG-06): `load_dc(op, p)` and
/// `load_dc(op, p+dp)`, differenced. The voltages never move (no re-solve);
/// device state is checkpointed so two loads do not corrupt the limiter.
fn perturb(
    device: &mut dyn crate::core::element::Element,
    param: &str,
    value: f64,
    op: &[f64],
    ref_map: &HashMap<usize, AnalogReference>,
    ctx: &Context,
) -> Result<Vec<Stamp<AnalogReference, f64>>> {
    let dp = step(value);
    let digital: &[LogicValue] = &[];
    let mut buf = CircularArrayBuffer2::new(1, op.len());
    let row = Array1::from_vec(op.to_vec());
    buf.push(&row.view());
    let dc_state = DcAnalysisState::new(&buf, digital, 1.0);

    let checkpoint = device.checkpoint_state();
    let nominal = device.load_dc(&dc_state, ctx);
    match device.set_param(param, Value::Real(value + dp)) {
        Ok(Invalidation::Restamp) | Ok(Invalidation::None) => {}
        Ok(_) => {
            // Crossing into Rebuild mid-perturbation: undo and fail loud.
            let _ = device.set_param(param, Value::Real(value));
            if let Some(cp) = checkpoint {
                device.restore_state(&cp);
            }
            return Err(Error::simple(
                SolverDomain::Sens,
                format!("perturbing `{param}` crosses a rebuild boundary"),
            ));
        }
        Err(ParamError::Unknown(_)) => {}
        Err(e) => return Err(Error::simple(SolverDomain::Sens, format!("param write: {e}"))),
    }
    let perturbed = device.load_dc(&dc_state, ctx);
    // Restore the parameter and any non-accept-gated device state.
    let _ = device.set_param(param, Value::Real(value));
    if let Some(cp) = checkpoint {
        device.restore_state(&cp);
    }

    Ok(diff_residual(&nominal, &perturbed, op, dp, ref_map))
}

/// `∂F/∂param = ((A_pert − A_nom)·V_op − (b_pert − b_nom)) / dp` per MNA row,
/// as `Stamp::Rhs` entries. The residual is `A·V − b`; differencing the two
/// stamp sets at the fixed operating point isolates the parameter's effect
/// without touching the solution.
fn diff_residual(
    nominal: &[Stamp<AnalogReference, f64>],
    perturbed: &[Stamp<AnalogReference, f64>],
    op: &[f64],
    dp: f64,
    ref_map: &HashMap<usize, AnalogReference>,
) -> Vec<Stamp<AnalogReference, f64>> {
    #[derive(Hash, Eq, PartialEq)]
    enum Key {
        Rhs(usize),
        Matrix(usize, usize),
    }
    let mut nom: HashMap<Key, f64> = HashMap::new();
    let accum = |stamps: &[Stamp<AnalogReference, f64>], into: &mut HashMap<Key, f64>| {
        for s in stamps {
            match s {
                Stamp::Rhs(r, v) => {
                    if let Some(i) = r.idx() {
                        *into.entry(Key::Rhs(i)).or_default() += *v;
                    }
                }
                Stamp::Matrix(r, c, v) => {
                    if let (Some(ri), Some(ci)) = (r.idx(), c.idx()) {
                        *into.entry(Key::Matrix(ri, ci)).or_default() += *v;
                    }
                }
            }
        }
    };
    accum(nominal, &mut nom);
    let mut pert: HashMap<Key, f64> = HashMap::new();
    accum(perturbed, &mut pert);

    let mut contrib: HashMap<usize, f64> = HashMap::new();
    let apply = |key: &Key, delta: f64, contrib: &mut HashMap<usize, f64>| {
        if delta == 0.0 {
            return;
        }
        match key {
            // Residual = A·V − b → −Δb contributes −delta.
            Key::Rhs(i) => *contrib.entry(*i).or_default() -= delta,
            // ΔA contributes ΔA·V_op.
            Key::Matrix(ri, ci) => {
                let vj = op.get(*ci).copied().unwrap_or(0.0);
                *contrib.entry(*ri).or_default() += delta * vj;
            }
        }
    };
    for (key, pv) in &pert {
        apply(key, pv - nom.get(key).copied().unwrap_or(0.0), &mut contrib);
    }
    for (key, nv) in &nom {
        if !pert.contains_key(key) {
            apply(key, -nv, &mut contrib);
        }
    }

    let mut out: Vec<Stamp<AnalogReference, f64>> = Vec::new();
    for (i, c) in contrib {
        let v = c / dp;
        if v != 0.0
            && let Some(r) = ref_map.get(&i)
        {
            out.push(Stamp::Rhs(r.clone(), v));
        }
    }
    out
}
