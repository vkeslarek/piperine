//! DC parameter-sensitivity stamp collection (dv-gradients DVG-04/06/07): the
//! `∂F/∂param` collector that, at the converged operating point, asks each
//! device instance for its parameter derivative — analytically through the
//! partials its shape declares against the param (DVG-04/05), by differencing
//! two evaluations otherwise (DVG-06).
//!
//! The fallback differences two assemblies at the **same** operating point
//! (the parameter is perturbed, the voltages are not) — never a re-solve — so
//! the adjoint economy (one transpose solve for every parameter) survives: only
//! `∂F/∂p` becomes approximate, never the solve count.
//!
//! Every derivative carries the engine that produced it, and the collector
//! names each instance that needed the fallback, by path (DVG-07) — never a
//! silent accuracy change. The DC adjoint driver (T8) wraps this collector with
//! the real transpose solve (T7).

use std::collections::{HashMap, HashSet};

use piperine_abi::device::Access;
use piperine_abi::raw::{Cell, cell, leaf, part, pin_flag, role};
use piperine_analog_solver::device::Ambient;
use piperine_digital_solver::net::NetId;
use crate::core::bind::{Bound, Site};

use crate::analyses::context::Context;
use piperine_analog_solver::netlist::AnalogReference;
use crate::core::circuit::CircuitInstance;
use crate::core::result::Result;
use piperine_analog_solver::error::{Error, SolverDomain};
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
    /// Analytic `∂F/∂p` from the partials the shape declares against the param.
    Analytic,
    /// Stamp perturbation at the operating point — no declared path reaches a term.
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
    /// Skip the declared partials of a circuit of devices and always difference
    /// by re-evaluation (the oracle of the declared route).
    force_fallback: bool,
}

impl<'a> ParamStampCollector<'a> {
    pub fn new(circuit: &'a mut CircuitInstance, context: Context) -> Self {
        Self { circuit, context, fallback_users: HashSet::new(), force_fallback: false }
    }

    /// For a circuit of devices: difference by re-evaluation only, ignoring the
    /// declared partials against params.
    pub fn finite_difference_only(mut self) -> Self {
        self.force_fallback = true;
        self
    }

    /// Collect `∂F/∂param` for `req` at the converged operating point `op`
    /// (node voltages indexed by [`AnalogReference::idx`]). No re-solve: the
    /// analytic path reads the cached kernel; the fallback re-stamps at the
    /// same voltages. Fails loud (reusing `.sens`'s rule) for an unknown
    /// element/parameter, a rebuild-class parameter, or a non-Real parameter.
    pub fn collect(&mut self, req: &ParamRequest, op: &[f64]) -> Result<ParamDerivative> {
        self.collect_device(req, op)
    }

    /// `∂F/∂param` of a circuit of devices. The param is addressed by instance
    /// path and `PARAM` pin name. The declared route seeds the partials against
    /// the param's pin and, chained through the declared partials of the
    /// discrete functions between it and the terms, against every pin the
    /// param reaches (abi.md §10 "Host procedures"); where no declared path
    /// reaches a term the derivative is a finite difference by re-evaluation at
    /// the same operating point (F5). A `STRUCTURAL` param fails loud: a
    /// derivative across a rebuild is not a sensitivity.
    fn collect_device(&mut self, req: &ParamRequest, op: &[f64]) -> Result<ParamDerivative> {
        let tolerances = self.context.tolerances.clone();
        let ref_map = index_refs(self.circuit.netlist().all_references());
        let sens = |detail: String| Error::simple(SolverDomain::Sens, detail);
        let bound = &mut self.circuit.abi;
        let Some(inst) = bound.names.instances.iter().position(|p| *p == req.label) else {
            return Err(sens(format!("no instance `{}`", req.label)));
        };
        let shape = bound.analog.plan().shape(inst);
        let params: Vec<&str> = shape.pins.iter().filter(|p| p.role == role::PARAM).map(|p| p.name.as_str()).collect();
        let Some(pin) = shape.pins.iter().find(|p| p.role == role::PARAM && p.name.as_str() == req.param) else {
            return Err(sens(format!(
                "`{}` declares no parameter `{}`; available: {}",
                req.label,
                req.param,
                if params.is_empty() { "(none)".to_string() } else { params.join(", ") }
            )));
        };
        if pin.flags & pin_flag::STRUCTURAL != 0 {
            return Err(sens(format!(
                "`{}.{}` is STRUCTURAL — sensitivities are only defined for params a host may write during a run",
                req.label, req.param
            )));
        }
        let real = pin.layout.fields.iter().next().is_some_and(|f| f.leaf == leaf::F64);
        if !real {
            return Err(sens(format!("`{}.{}` is not Real-typed — only Real parameters are differentiable", req.label, req.param)));
        }
        let net = bound.names.params[&(req.label.clone(), req.param.clone())];
        let amb = Ambient { time: 0.0, freq: 0.0, is_static: true, gmin: tolerances.gmin, source_scale: 1.0 };

        if !self.force_fallback {
            let derivs = net_derivatives(bound, net)?;
            let mut stamps = Vec::new();
            for j in 0..bound.names.instances.len() {
                let wrt: Vec<(u32, f64)> = bound.names.pins[j]
                    .iter()
                    .enumerate()
                    .filter_map(|(pi, site)| match site {
                        Site::Net(n) => derivs.get(n).map(|d| (pi as u32, *d)),
                        _ => None,
                    })
                    .collect();
                if !wrt.is_empty() {
                    stamps.extend(bound.analog.param_stamps(j, &wrt, op, amb)?);
                }
            }
            if !stamps.is_empty() {
                return Ok(ParamDerivative {
                    label: req.label.clone(),
                    param: req.param.clone(),
                    stamps,
                    engine: SensitivityEngine::Analytic,
                });
            }
        }

        // No declared path (or forced): difference two evaluations at the same iterate.
        self.fallback_users.insert(req.label.clone());
        let Some(slot) = bound.names.slots.get(&net).copied() else {
            // No analog function reads the param: the residual does not depend on it.
            return Ok(ParamDerivative {
                label: req.label.clone(),
                param: req.param.clone(),
                stamps: Vec::new(),
                engine: SensitivityEngine::StampPerturbation,
            });
        };
        // The param's present value: the host's last write, else the net's `init`.
        let init = bound
            .host_writes
            .iter()
            .rev()
            .find(|(n, _)| *n == net)
            .map(|(_, b)| b.clone())
            .unwrap_or_else(|| bound.digital.plan().nets[net.0 as usize].init.clone());
        let value = init.get(..8).and_then(|b| b.try_into().ok()).map_or(0.0, f64::from_le_bytes);
        let dp = step(value);
        // Central difference: the same iterate, the param moved by `±dp`.
        let mut at = |v: f64| -> Result<Vec<Stamp<AnalogReference, f64>>> {
            let mut moved = init.clone();
            moved[..8].copy_from_slice(&v.to_le_bytes());
            bound.analog.set_discrete(slot, &moved);
            bound.analog.assemble_dc(op, amb)
        };
        let (plus, minus) = (at(value + dp), at(value - dp));
        bound.analog.set_discrete(slot, &init);
        let stamps = diff_residual(&minus?, &plus?, op, 2.0 * dp, &ref_map);
        Ok(ParamDerivative {
            label: req.label.clone(),
            param: req.param.clone(),
            stamps,
            engine: SensitivityEngine::StampPerturbation,
        })
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

/// The derivative of every net's bytes with respect to the param on `net`: 1 on the
/// param's own net, and across each discrete function the sum over its declared
/// partials (read net derivative times the partial), evaluated at the engine's
/// present net values.
fn net_derivatives(bound: &Bound, net: NetId) -> Result<HashMap<NetId, f64>> {
    let plan = bound.digital.plan();
    let mut edges: Vec<(NetId, NetId, f64)> = Vec::new();
    for node in plan.analysis().functions() {
        let inst = &plan.instances[node.instance as usize];
        let shape = inst.handle.shape();
        let f = shape.functions.get(node.k as usize).expect("an analyzed function");
        if f.partials.is_empty() {
            continue;
        }
        let net_of = |c: &Cell| match (c.kind == cell::PIN).then(|| bound.names.pins[node.instance as usize].get(c.index as usize)) {
            Some(Some(Site::Net(n))) => Some(*n),
            _ => None,
        };
        let sens = |detail: String| Error::simple(SolverDomain::Sens, detail);
        let reads = shape.cell_lens(&f.reads, Access::FunctionRead).ok_or_else(|| sens(format!("{}: bad read cells", inst.path)))?;
        let writes = shape.cell_lens(&f.writes, Access::FunctionWrite).ok_or_else(|| sens(format!("{}: bad write cells", inst.path)))?;
        let inputs: Vec<Vec<u8>> = f
            .reads
            .iter()
            .zip(&reads)
            .map(|(c, len)| match net_of(c).filter(|_| c.part == part::VALUE) {
                Some(n) => {
                    let mut b = bound.digital.read(n).to_vec();
                    b.resize(*len, 0);
                    b
                }
                None => vec![0; *len],
            })
            .collect();
        let input: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
        let mut out: Vec<Vec<u8>> = writes.iter().map(|n| vec![0; *n]).collect();
        let mut output: Vec<&mut [u8]> = out.iter_mut().map(Vec::as_mut_slice).collect();
        let mut jac = vec![0.0; f.partials.len()];
        let seed = vec![1u8; f.partials.len()];
        inst.handle
            .eval(node.k, &input, &mut output, &seed, &mut jac)
            .map_err(|e| sens(format!("instance `{}`: function `{}`: {e}", inst.path, f.name.as_str())))?;
        for (pi, p) in f.partials.iter().enumerate() {
            let (Some(r), Some(w)) = (f.reads.get(p.read as usize), f.writes.get(p.write as usize)) else { continue };
            if let (Some(rn), Some(wn)) = (net_of(r), net_of(w)) {
                edges.push((rn, wn, jac[pi]));
            }
        }
    }
    let mut derivs: HashMap<NetId, f64> = HashMap::from([(net, 1.0)]);
    for _ in 0..=edges.len() {
        let mut next: HashMap<NetId, f64> = HashMap::from([(net, 1.0)]);
        for (rn, wn, g) in &edges {
            if let Some(d) = derivs.get(rn) {
                *next.entry(*wn).or_default() += g * d;
            }
        }
        if next == derivs {
            break;
        }
        derivs = next;
    }
    Ok(derivs)
}
