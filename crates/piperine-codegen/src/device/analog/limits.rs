//! Limits capability (device side): `$limit` (pnjlim/fetlim) vold-slot
//! bookkeeping — the pre-stamp voltage transform and the seed/update
//! machinery that makes junction devices converge.
//!
//! The `active`/`active_slots`/`seeds` bookkeeping (rules 2 and 4 — seed
//! preservation, convergence veto) delegates to
//! `piperine_signal::limit::Limiter::advance_precomputed`. Rule 3's formula
//! itself (pnjlim/fetlim/limvds) is **not** recomputed here: since T4/T5 the
//! JIT already calls `signal::limit::{pnjlim,fetlim,limvds}` directly
//! (`kernel.eval_limit_update`), so `vlim` arrives pre-computed. `vold`
//! stays in this instance's shared analog `state` bank (the JIT's own
//! `$limit` calls read it from there — `advance_precomputed` never keeps a
//! copy of its own), which is also why checkpointing splits across two
//! places: `Limiter::pack`/`unpack` carry `active`+`seeds`, the caller packs
//! the `state` bank's vold slots alongside it (unchanged by this file).

use crate::kernel::analog::AnalogKernel;
use crate::emit::abi::SimCtx;

use super::LoadCtx;

/// Per-instance `$limit` runtime state: whether limiting is still moving,
/// and the critical-voltage seeds used to tell an unbiased (still-seeded)
/// junction from a tracked one. A thin wrapper: `piperine_signal::limit::
/// Limiter` owns `active`/`active_slots`/`seeds`; `vold` lives in the
/// instance's `state` bank, passed in by every call (never inside the
/// wrapped type).
pub(super) struct Limiter {
    inner: piperine_signal::limit::Limiter,
}

impl Limiter {
    /// A fresh limiter with `num_limits` unseeded vold slots. Call
    /// [`Limiter::seed`] once the instance's state bank exists to fill in
    /// the critical-voltage seeds (kept as two steps so seeding runs at the
    /// same point in `AnalogInstance::new` as before this split — after
    /// `@initial` events, matching the original construction order exactly).
    pub(super) fn new(num_limits: usize) -> Self {
        Self { inner: piperine_signal::limit::Limiter::new(vec![0.0; num_limits]) }
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
        self.inner = piperine_signal::limit::Limiter::new(seeds);
    }

    pub(super) fn active(&self) -> bool {
        self.inner.active()
    }

    /// The first slot still clamping at the last [`update`](Self::update), for
    /// naming the limiter that actually fired in `limiting_report` (PIA-15/17).
    /// `None` when no slot is active (the report is then suppressed).
    pub(super) fn active_slot(&self) -> Option<usize> {
        self.inner.active_slot()
    }

    /// Pack the limiter's own state (the `active` flag + the vcrit `seeds`)
    /// for a checkpoint (ABI-04). The vold slots live in the analog
    /// instance's `state` bank and are packed by the caller alongside this.
    /// Layout: `[active_as_f64, seed_0, seed_1, …]`.
    pub(super) fn pack(&self) -> Vec<f64> {
        let seeds = self.inner.seeds();
        let mut v = Vec::with_capacity(1 + seeds.len());
        v.push(if self.inner.active() { 1.0 } else { 0.0 });
        v.extend_from_slice(seeds);
        v
    }

    /// Restore the limiter's own state from a pack produced by
    /// [`pack`](Self::pack) (ABI-04). No-op when the layout is too short.
    pub(super) fn unpack(&mut self, packed: &[f64]) {
        let Some((&a, rest)) = packed.split_first() else { return };
        if rest.len() == self.inner.seeds().len() {
            self.inner.restore_seeds(rest.to_vec(), a != 0.0);
        }
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
    ///
    /// `vlim` (the JIT's already-limited value — rule 3, "limit against the
    /// previous limited value", is applied by the JIT reading `vold` from
    /// `state` on its own next call) and `vnew` (the raw unlimited value)
    /// both come from the compiled kernel; rules 2 (seed preservation) and 4
    /// (convergence veto) are `signal::limit::Limiter::advance_precomputed`'s
    /// job, applied directly over this instance's own `state` slice.
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
        self.inner.advance_precomputed(&vlim, &vnew, &mut state[base..base + nl]);
    }
}
