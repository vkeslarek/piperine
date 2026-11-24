//! Run tolerances and per-analysis convergence tunables: `Tolerances` (immutable
//! per-run numerics plus the convergence tests that read only them) and
//! `Policy` (the mutable convergence tunables a driver owns, MD-04).
//!
//! Distinct from `config.rs`, which is pure data — the tunable
//! schedule/gain/trace literals with no logic beyond construction.

use crate::config::TraceFlags;
use crate::netlist::Netlist;
use ndarray::ArrayView1;

// ── Tolerances (immutable, Clone) ───────────────────────────────────────────

/// Immutable per-run numerical tolerances. Shared across every analysis
/// through `Context`. Extracted from the old flat `Context` fields (MD-04).
///
/// `Clone`, not `Copy` (as of T48/LRMW2-53): `node_vntol_overrides` is an
/// `Rc<HashMap<..>>`, needed for §6.5.8's per-node nature-`abstol`
/// inheritance ("The node shall be treated as having a potential abstol
/// with a value equal to the smallest abstol of all the potential natures
/// of all the disciplines with which it is associated") — a genuinely
/// per-net override the single flat `vntol` scalar cannot express. `Rc`
/// keeps a clone of `Tolerances` (still handed around by value at every
/// call site that used to rely on `Copy`) cheap — one pointer bump, not a
/// map copy.
#[derive(Debug, Clone)]
pub struct Tolerances {
    pub gmin: f64,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub min_res: f64,
    /// Truncation error tolerance for adaptive timestep (default: 7.0)
    pub trtol: f64,
    /// Charge tolerance in Coulombs for truncation error (default: 1e-14)
    pub chgtol: f64,
    pub temperature: f64,
    /// Circuit-wide diagonal conductance to ground on every node (default 0).
    /// Helps convergence on floating/poorly-damped topologies (ngspice gshunt).
    pub gshunt: f64,
    /// Per-node potential-`abstol` override (§6.5.8, LRMW2-53, T48) — a
    /// node present here uses this value instead of `vntol` in both
    /// `has_converged`'s update test and `residual_test`'s residual
    /// check, which is what makes a tighter nature `abstol` *observably*
    /// tighten convergence rather than merely being carried and ignored.
    /// Empty by default (every node uses the flat `vntol`, unchanged
    /// behavior). A node absent here is unaffected — `None` means "no
    /// declared-nature information reached this node," not "abstol is
    /// zero."
    ///
    /// Flow-nature `abstol` inheritance (§6.5.8's other half, "a flow
    /// abstol... of all the flow natures") is not covered by this field
    /// — a disclosed scope cut, not silently dropped: LRMW2-53's
    /// Done-when asks for "a tighter nature abstol observably tightens
    /// convergence on the connected net," which the potential/node case
    /// alone already demonstrates.
    pub node_vntol_overrides: std::rc::Rc<std::collections::HashMap<crate::netlist::NodeIdentifier, f64>>,
    /// Per-branch flow-`abstol` override (§17.1 rules 1 and 2): a branch unknown present here
    /// uses the smaller of its nature's flow `abstol` and `abstol`, in the update test and in the
    /// residual test of its row. Empty by default; a branch absent here is unaffected.
    pub branch_abstol_overrides: std::rc::Rc<std::collections::HashMap<crate::netlist::BranchIdentifier, f64>>,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            gmin: 1e-12,
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            min_res: 1e-12,
            trtol: 7.0,
            chgtol: 1e-14,
            temperature: 300.15,
            gshunt: 0.0,
            node_vntol_overrides: std::rc::Rc::new(std::collections::HashMap::new()),
            branch_abstol_overrides: std::rc::Rc::new(std::collections::HashMap::new()),
        }
    }
}

impl Tolerances {
    /// The absolute tolerance of a node: the smaller of the session's `vntol` and its nature's
    /// potential `abstol` (in `node_vntol_overrides`, §6.5.8); a nature can only tighten the
    /// session value (§17.1), and `self.vntol` stands for a node absent from the overrides.
    fn vntol_for(&self, variable: &crate::netlist::AnalogVariable) -> f64 {
        match variable {
            crate::netlist::AnalogVariable::Node(id) => {
                self.node_vntol_overrides.get(id).map_or(self.vntol, |nature| nature.min(self.vntol))
            }
            _ => self.vntol,
        }
    }

    /// The absolute tolerance of a branch unknown and of its row: the smaller of the session's
    /// `abstol` and its nature's flow `abstol` (in `branch_abstol_overrides`); a nature can only
    /// tighten the session value (§17.1).
    fn abstol_for(&self, variable: &crate::netlist::AnalogVariable) -> f64 {
        match variable {
            crate::netlist::AnalogVariable::Branch(id) => self.branch_abstol_overrides.get(id).map_or(self.abstol, |nature| nature.min(self.abstol)),
            _ => self.abstol,
        }
    }

    /// The update test of one unknown: `|new − old| ≤ reltol·max(|old|, |new|) + abs`.
    fn update_within(&self, reference: &crate::netlist::AnalogReference, old_v: f64, new_v: f64) -> bool {
        let abs_limit = if reference.is_branch() {
            self.abstol_for(reference.variable())
        } else {
            self.vntol_for(reference.variable())
        };
        let magnitude = old_v.abs().max(new_v.abs());
        let allowed_error = self.reltol * magnitude + abs_limit;
        (new_v - old_v).abs() <= allowed_error
    }

    /// The residual test of one row: `|residual| ≤ abs + reltol·scale`.
    fn residual_within(&self, reference: &crate::netlist::AnalogReference, residual: f64, scale: f64) -> bool {
        let abs_limit = if reference.variable().is_branch() {
            self.abstol_for(reference.variable())
        } else {
            self.vntol_for(reference.variable())
        };
        !(residual.abs() > abs_limit + self.reltol * scale)
    }

    /// The convergence test that used to be `Context::has_converged` — moved
    /// here because it only reads tolerance fields. Same logic, same output.
    pub fn has_converged(
        &self,
        old_values_opt: Option<ArrayView1<f64>>,
        new_values: &ArrayView1<f64>,
        netlist: &Netlist,
    ) -> bool {
        let Some(old_values) = old_values_opt else { return false; };

        netlist
            .all_references()
            .iter()
            .filter(|s| s.idx().is_some())
            .all(|reference| {
                let index = reference.idx().unwrap();

                if index >= old_values.len() || index >= new_values.len() {
                    return true;
                }

                self.update_within(reference, old_values[index], new_values[index])
            })
    }

    /// ngspice `NIconvTest`: every node's current imbalance (and every branch
    /// row's equation residual) must be within tolerance.
    pub fn residual_test(
        &self,
        netlist: &Netlist,
        residual: &[f64],
        scale: &[f64],
    ) -> bool {
        use crate::math::linear::AsIndex;
        for r in netlist.all_references() {
            let Some(i) = r.as_index() else { continue };
            if i >= residual.len() {
                continue;
            }
            if !self.residual_within(r, residual[i], scale[i]) {
                return false;
            }
        }
        true
    }

    /// The unknowns that fail the update test of [`has_converged`](Self::has_converged)
    /// or the residual test of [`residual_test`](Self::residual_test), in index
    /// order: what a Newton exhaustion names (§17.1).
    pub fn failing(
        &self,
        old_values: ArrayView1<f64>,
        new_values: &ArrayView1<f64>,
        netlist: &Netlist,
        residual: &[f64],
        scale: &[f64],
    ) -> Vec<usize> {
        use crate::math::linear::AsIndex;
        let mut failing: Vec<usize> = netlist
            .all_references()
            .into_iter()
            .filter_map(|reference| {
                let i = reference.as_index()?;
                let update_fails = i < old_values.len()
                    && i < new_values.len()
                    && !self.update_within(reference, old_values[i], new_values[i]);
                let residual_fails = i < residual.len()
                    && i < scale.len()
                    && !self.residual_within(reference, residual[i], scale[i]);
                (update_fails || residual_fails).then_some(i)
            })
            .collect();
        failing.sort_unstable();
        failing
    }
}

// ── Policy (mutable, owned by drivers/plan) ────────────────────────────────

/// Convergence tunables the Newton loop consults each solve. Owned by the
/// driver (each analysis solver carries its own), never by the shared
/// immutable `Context` (MD-04). Hosts configure it per analysis.
#[derive(Debug, Clone)]
pub struct Policy {
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
    /// Diagnostic trace toggles (SS-08). `Default` seeds them from the
    /// `PIPERINE_TRACE_{GMIN,SRC,TRAN}` env vars — the single env read left;
    /// every trace site reads these typed fields.
    pub trace: TraceFlags,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_iter: 500,
            dc_damp_tolerance: 0.5,
            trace: TraceFlags::from_env(),
        }
    }
}

