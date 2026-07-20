//! Limits capability (device side): `$limit` (pnjlim/fetlim) vold-slot
//! bookkeeping — the pre-stamp voltage transform and the seed/update
//! machinery that makes junction devices converge.

use crate::kernel::analog::AnalogKernel;
use crate::emit::abi::SimCtx;

use super::LoadCtx;

/// Per-instance `$limit` runtime state: whether limiting is still moving,
/// and the critical-voltage seeds used to tell an unbiased (still-seeded)
/// junction from a tracked one.
pub(super) struct Limiter {
    /// Whether junction voltage limiting was still moving at the last load
    /// (vetoes Newton convergence — see [`Limiter::update`]).
    active: bool,
    /// Per-`$limit` seed voltage `vcrit`.
    seeds: Vec<f64>,
}

impl Limiter {
    /// A fresh limiter with `num_limits` unseeded vold slots. Call
    /// [`Limiter::seed`] once the instance's state bank exists to fill in
    /// the critical-voltage seeds (kept as two steps so seeding runs at the
    /// same point in `AnalogInstance::new` as before this split — after
    /// `@initial` events, matching the original construction order exactly).
    pub(super) fn new(num_limits: usize) -> Self {
        Self { active: false, seeds: vec![0.0; num_limits] }
    }

    /// Seed each `$limit` vold slot (in `state`) with its critical voltage
    /// `vcrit`, so a junction starts limiting near turn-on (ngspice
    /// MODEINITJCT) rather than from 0 V. `vcrit` depends only on
    /// params/temperature, not node voltages.
    pub(super) fn seed(&mut self, kernel: &AnalogKernel, num_terminals: usize, params: &[f64], state: &mut [f64], vars: &[f64], sim: &SimCtx) {
        let nl = kernel.num_limits();
        if nl == 0 {
            return;
        }
        let base = kernel.limit_base();
        let zeros = vec![0.0; num_terminals];
        let mut seeds = vec![0.0; nl];
        kernel.eval_limit_seed(&zeros, params, state, vars, sim, &mut seeds);
        for (i, s) in seeds.iter().enumerate() {
            state[base + i] = *s;
        }
        self.seeds = seeds;
    }

    pub(super) fn active(&self) -> bool {
        self.active
    }

    /// Node voltages with each `$limit` junction branch replaced by its
    /// limited value `vlim` — the linearization point for the Norton
    /// transform when voltage limiting is active. Non-junction nodes are
    /// unchanged. Returns `volts` unchanged when the device has no `$limit`.
    pub(super) fn limited_volts(&self, cx: &LoadCtx<'_>, volts: &[f64]) -> Vec<f64> {
        let nl = cx.kernel.num_limits();
        if nl == 0 {
            return volts.to_vec();
        }
        let mut vlim = vec![0.0; nl];
        cx.kernel.eval_limit_update(volts, cx.params, cx.state, cx.vars, cx.sim, &mut vlim);
        let mut vnew = vec![0.0; nl];
        cx.kernel.eval_limit_vnew(volts, cx.params, cx.state, cx.vars, cx.sim, &mut vnew);
        let mut veff = volts.to_vec();
        for (i, branch) in cx.kernel.limit_branches().iter().enumerate() {
            let Some((plus, minus)) = branch else { continue };
            let vp = plus.map_or(0.0, |p| volts[p]);
            let vm = minus.map_or(0.0, |m| volts[m]);
            let vbr_raw = vp - vm;
            // `vnew = type · vbr_raw`, type = ±1: recover the branch polarity so
            // the limited node-space voltage is `vlim / type`.
            let ty = if vbr_raw.abs() > 1e-12 { (vnew[i] / vbr_raw).signum() } else { 1.0 };
            let vbr_eff = vlim[i] * ty;
            // Move the minus node if it is a real node (keeps a shared plus node
            // — e.g. a BJT base' — fixed); otherwise move the plus node.
            if let Some(m) = minus {
                veff[*m] = vp - vbr_eff;
            } else if let Some(p) = plus {
                veff[*p] = vm + vbr_eff;
            }
        }
        veff
    }

    /// Advance the `$limit` vold slots (in `state`) after loading: store
    /// this iteration's limited voltages so the next Newton iteration limits
    /// against them (ngspice stores the limited junction voltage in device
    /// state). This is what makes junction devices converge — without it a
    /// stiff exponential overshoots and stalls. Called each iteration of DC
    /// and transient loads; AC/noise reuse the converged DC vold (limiter
    /// inactive there).
    pub(super) fn update(&mut self, kernel: &AnalogKernel, volts: &[f64], params: &[f64], state: &mut [f64], vars: &[f64], sim: &SimCtx) {
        let nl = kernel.num_limits();
        if nl == 0 {
            return;
        }
        let base = kernel.limit_base();
        let mut vlim = vec![0.0; nl];
        kernel.eval_limit_update(volts, params, state, vars, sim, &mut vlim);
        let mut vnew = vec![0.0; nl];
        kernel.eval_limit_vnew(volts, params, state, vars, sim, &mut vnew);
        // A junction is "still limiting" iff pnjlim actually clamped this
        // iteration — the limited value differs from the raw branch voltage
        // (ngspice's `Check == 1`). While that holds, the Newton loop must not
        // declare convergence (see PiperineDevice::limiting_active): a clamped
        // junction can momentarily satisfy KCL at a non-solution voltage. Tiny
        // Newton jitter once limiting is off (vnew ≈ vlim) must NOT veto, hence
        // the tolerance below.
        let mut active = false;
        for (i, v) in vlim.into_iter().enumerate() {
            let old = state[base + i];
            // Preserve the vcrit seed until the junction is first biased:
            // `pnjlim(0, vcrit) = 0` on the opening iterations would discard the
            // seed and let the node float to the supply (ngspice MODEINITJCT).
            let seeded = (old - self.seeds[i]).abs() <= 1e-12;
            if seeded && v < old {
                continue;
            }
            if (vnew[i] - v).abs() > 1e-6 + 1e-4 * vnew[i].abs() {
                active = true;
            }
            state[base + i] = v;
        }
        self.active = active;
    }
}
