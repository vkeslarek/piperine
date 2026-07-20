//! Forces capability (device side): the per-instance branch-current
//! unknowns for `V`/`I`-source forces, plus their MNA stamping.

use piperine_solver::abi::{AnalogReference, BranchIdentifier, Netlist, Stamp};

use crate::kernel::analog::AnalogKernel;
use crate::resolve::NodeId;

use super::{LoadCtx, Stamps};

/// One MNA branch-current unknown per force row, and the stamping that
/// couples them into the netlist.
pub(super) struct ForceStamper {
    refs: Vec<AnalogReference>,
}

impl ForceStamper {
    /// Allocate one branch-current unknown per kernel force row.
    pub(super) fn new(label: &str, kernel: &AnalogKernel, netlist: &mut Netlist) -> Self {
        let refs = (0..kernel.num_forces())
            .map(|i| netlist.connect_branch(BranchIdentifier::new(label, format!("force{i}"))))
            .collect();
        Self { refs }
    }

    pub(super) fn refs(&self) -> &[AnalogReference] {
        &self.refs
    }

    /// The force branch whose terminals are `(tp, tm)`, with the sign flip
    /// when the probe orientation is reversed — the branch-current column a
    /// flux/impedance term couples to.
    pub(super) fn branch_target(&self, kernel: &AnalogKernel, tp: NodeId, tm: NodeId) -> Option<(usize, f64)> {
        let force_terminals = kernel.force_terminals();
        force_terminals
            .iter()
            .position(|&(p, m)| p == tp && m == tm)
            .map(|k| (k, 1.0))
            .or_else(|| {
                force_terminals
                    .iter()
                    .position(|&(p, m)| p == tm && m == tp)
                    .map(|k| (k, -1.0))
            })
    }
}

impl Stamps for ForceStamper {
    /// Ideal-source rows: per force `i`, a branch-current unknown `ib_i`,
    /// KCL coupling at its terminals, and the branch equation
    /// `V(p) − V(m) − E_i(V) = 0`, Newton-linearised.
    fn stamp(&self, cx: &LoadCtx<'_>, volts: &[f64], src_scale: f64) -> Vec<Stamp<AnalogReference, f64>> {
        let nf = cx.kernel.num_forces();
        if nf == 0 {
            return Vec::new();
        }
        let n = cx.node_refs.len();
        let mut e = vec![0.0; nf];
        let mut de = vec![0.0; nf * n];
        cx.kernel.eval_force(volts, cx.params, cx.state, cx.vars, cx.sim, &mut e);
        cx.kernel.eval_force_jacobian(volts, cx.params, cx.state, cx.vars, cx.sim, &mut de);
        // Source stepping: scale the forced value (and its bias dependence) by
        // the independent-source factor. Internal-node-collapse forces
        // (`V(c,cp) <- 0`) have `e = 0`, so they are untouched; only real
        // driven voltages ramp. `1.0` in normal operation.
        if src_scale != 1.0 {
            for v in &mut e {
                *v *= src_scale;
            }
            for v in &mut de {
                *v *= src_scale;
            }
        }

        let mut stamps = Vec::new();
        for (i, (branch, &(plus, minus))) in self
            .refs
            .iter()
            .zip(cx.kernel.force_terminals().iter())
            .enumerate()
        {
            let plus_ref = cx.terminal_ref(plus);
            let minus_ref = cx.terminal_ref(minus);
            // KCL: ib leaves `plus`, enters `minus`.
            if let Some(p) = &plus_ref {
                stamps.push(Stamp::Matrix(p.clone(), branch.clone(), 1.0));
                stamps.push(Stamp::Matrix(branch.clone(), p.clone(), 1.0));
            }
            if let Some(m) = &minus_ref {
                stamps.push(Stamp::Matrix(m.clone(), branch.clone(), -1.0));
                stamps.push(Stamp::Matrix(branch.clone(), m.clone(), -1.0));
            }
            // Controlled-source coupling: −∂E/∂V_j on the branch row.
            let mut rhs = e[i];
            for j in 0..n {
                let g = de[i * n + j];
                if g == 0.0 {
                    continue;
                }
                if let Some(col) = &cx.node_refs[j] {
                    stamps.push(Stamp::Matrix(branch.clone(), col.clone(), -g));
                }
                rhs -= g * volts[j];
            }
            stamps.push(Stamp::Rhs(branch.clone(), rhs));
        }
        // Series-impedance terms `coeff·I(target)` split out of force values
        // (`V(p,n) <- R·I(p,n) + …`): the branch row gains `−coeff` on the
        // target branch-current column — an exact series resistor (perfect
        // short at `R = 0`), linear, entirely in the matrix. Deliberately
        // outside the `src_scale` scaling: an impedance is not a source.
        if cx.kernel.has_force_current() {
            let terms = cx.kernel.current_terms();
            let mut coeffs = vec![0.0; terms.len()];
            cx.kernel.eval_force_current(volts, cx.params, cx.state, cx.vars, cx.sim, &mut coeffs);
            for (&(force_idx, tp, tm), &r) in terms.iter().zip(&coeffs) {
                if r == 0.0 {
                    continue;
                }
                let Some((target_idx, sign)) = self.branch_target(cx.kernel, tp, tm) else { continue };
                stamps.push(Stamp::Matrix(
                    self.refs[force_idx].clone(),
                    self.refs[target_idx].clone(),
                    -r * sign,
                ));
            }
        }
        stamps
    }
}
