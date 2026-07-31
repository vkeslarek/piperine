//! The junction limiter — `$limit` (pnjlim/fetlim/limvds) convergence
//! machinery. The least guessable knowledge in the tree (`ideal.md` §3.9 D):
//! it matters to level 1 because a stiff `Behavior` (a high-gain
//! `comparator`, a sharp `switch`) hits exactly the failure these rules
//! exist to prevent.
//!
//! Extracted from `piperine-codegen`'s `device/analog/limits.rs::Limiter`
//! (the vold-slot bookkeeping) and `emit/analog_expr.rs`'s
//! `emit_pnjlim`/`emit_fetlim`/`emit_limvds` (the ngspice-faithful branchless
//! formulas, here as ordinary `if`/`else` — the two forms are equivalent
//! when every candidate value is already a plain finite computation, which
//! holds for all three). Codegen keeps its own JIT-emitted copy until
//! `codegen-simplification` delegates to this one.
//!
//! The five rules that must survive, unabridged:
//! 1. **Seed with `vcrit`, not zero.** [`Limiter::new`] seeds every slot at
//!    its critical voltage (params/temperature only, never a node voltage)
//!    so a junction opens near turn-on rather than at 0 V (ngspice
//!    `MODEINITJCT`).
//! 2. **Preserve the seed until first bias.** The update is skipped while a
//!    slot is still at its seed and the new value is lower — `pnjlim(0,
//!    vcrit) = 0` on the opening iterations would otherwise discard the
//!    seed and let the node float to the supply.
//! 3. **Limit against the previous *limited* value.** Each iteration stores
//!    its limited voltage for the next one to limit against — this is what
//!    makes junction devices converge; without it a stiff exponential
//!    overshoots and stalls.
//! 4. **Limiting vetoes convergence, with a tolerance.** A slot is "still
//!    clamping" iff the limited value differs from the raw branch voltage
//!    by more than `1e-6 + 1e-4·|vnew|`; while any slot clamps, Newton must
//!    not declare convergence. The tolerance keeps post-limiting jitter
//!    from vetoing forever.
//! 5. **Move the minus node, not the plus.** [`limited_volts`] displaces the
//!    minus node when it is a real node (keeping a shared plus node — a
//!    BJT's internal base — fixed), falling back to the plus node only when
//!    the minus is ground.

/// The `$limit` kind a slot uses, and its own (non-node) parameter:
/// `vte` for `pnjlim`, `vto` for `fetlim`. `Limvds` takes neither.
#[derive(Debug, Clone, Copy)]
pub enum LimitKind {
    Pnjlim { vte: f64 },
    Fetlim { vto: f64 },
    Limvds,
}

/// ngspice `MIN(a, b)` — a plain comparison, not IEEE `fmin`.
fn ng_min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// ngspice `MAX(a, b)`.
fn ng_max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// ngspice `DEVpnjlim(vnew, vold, vte, vcrit)` (devsup.c) — the pn-junction
/// voltage limiter.
pub fn pnjlim(vnew: f64, vold: f64, vte: f64, vcrit: f64) -> f64 {
    let dv = vnew - vold;
    if !(vnew > vcrit && dv.abs() > 2.0 * vte) {
        return vnew;
    }
    let arg = 1.0 + dv / vte;
    let posval = if arg > 0.0 { vold + vte * arg.ln() } else { vcrit };
    let negval = vte * (vnew / vte).ln();
    if vold > 0.0 { posval } else { negval }
}

/// ngspice `DEVfetlim(vnew, vold, vto)` (devsup.c) — the MOSFET
/// gate-voltage limiter.
pub fn fetlim(vnew: f64, vold: f64, vto: f64) -> f64 {
    let vold_m_vto = vold - vto;
    let vtsthi = (2.0 * vold_m_vto).abs() + 2.0;
    let vtstlo = vold_m_vto.abs() + 1.0;
    let vtox = vto + 3.5;
    let delv = vnew - vold;
    let neg_delv = -delv;

    let on = vold >= vto;
    let high = vold >= vtox;
    let delv_le0 = delv <= 0.0;

    let high_val = if delv_le0 {
        if vnew >= vtox {
            if neg_delv > vtstlo { vold - vtstlo } else { vnew }
        } else {
            ng_max(vnew, vto + 2.0)
        }
    } else if delv >= vtsthi {
        vold + vtsthi
    } else {
        vnew
    };

    let mid_val = if delv_le0 { ng_max(vnew, vto - 0.5) } else { ng_min(vnew, vto + 4.0) };

    let on_val = if high { high_val } else { mid_val };

    let off_val = if delv_le0 {
        if neg_delv > vtsthi { vold - vtsthi } else { vnew }
    } else {
        let vto_p05 = vto + 0.5;
        if vnew <= vto_p05 {
            if delv > vtstlo { vold + vtstlo } else { vnew }
        } else {
            vto_p05
        }
    };

    if on { on_val } else { off_val }
}

/// ngspice `DEVlimvds(vnew, vold)` (devsup.c) — the MOSFET drain-source
/// voltage limiter.
pub fn limvds(vnew: f64, vold: f64) -> f64 {
    let hi = vold >= 3.5;
    let gt = vnew > vold;

    let hi_val = if gt { ng_min(vnew, 3.0 * vold + 2.0) } else if vnew < 3.5 { ng_max(vnew, 2.0) } else { vnew };
    let lo_val = if gt { ng_min(vnew, 4.0) } else { ng_max(vnew, -0.5) };

    if hi { hi_val } else { lo_val }
}

impl LimitKind {
    /// The limited value for this kind, given the raw new voltage, the
    /// previous *limited* value (rule 3), and the slot's seed `vcrit`
    /// (used by `pnjlim`; ignored by `fetlim`/`limvds`, which have no
    /// junction critical voltage).
    fn limit(&self, vnew: f64, vold: f64, vcrit: f64) -> f64 {
        match *self {
            LimitKind::Pnjlim { vte } => pnjlim(vnew, vold, vte, vcrit),
            LimitKind::Fetlim { vto } => fetlim(vnew, vold, vto),
            LimitKind::Limvds => limvds(vnew, vold),
        }
    }
}

/// Per-instance `$limit` runtime state: whether limiting is still moving,
/// and the critical-voltage seeds used to tell an unbiased (still-seeded)
/// junction from a tracked one.
pub struct Limiter {
    active: bool,
    active_slots: Vec<bool>,
    seeds: Vec<f64>,
    vold: Vec<f64>,
}

impl Limiter {
    /// A fresh limiter, one slot per `seeds` entry (rule 1: seed with
    /// `vcrit`, not zero).
    pub fn new(seeds: Vec<f64>) -> Self {
        let vold = seeds.clone();
        let active_slots = vec![false; seeds.len()];
        Self { active: false, active_slots, seeds, vold }
    }

    /// Whether any slot is still clamping at the last [`update`](Self::update)
    /// — vetoes Newton convergence while `true` (rule 4).
    pub fn active(&self) -> bool {
        self.active
    }

    /// The first slot still clamping, for naming the limiter that actually
    /// fired. `None` when no slot is active.
    pub fn active_slot(&self) -> Option<usize> {
        self.active_slots.iter().position(|&a| a)
    }

    /// The current limited value per slot (what the caller linearizes
    /// against).
    pub fn limited(&self) -> &[f64] {
        &self.vold
    }

    /// Advance every slot given this iteration's raw branch voltages
    /// `vnew` (rules 2–4), storing the limited value for the next
    /// iteration to limit against (rule 3). Returns the limited values.
    pub fn update(&mut self, vnew: &[f64], kinds: &[LimitKind]) -> &[f64] {
        for slot in self.active_slots.iter_mut() {
            *slot = false;
        }
        let mut active = false;
        for i in 0..self.vold.len() {
            let old = self.vold[i];
            let v = kinds[i].limit(vnew[i], old, self.seeds[i]);
            // Rule 2: preserve the vcrit seed until the junction is first
            // biased — `pnjlim(0, vcrit) = 0` on the opening iterations
            // would otherwise discard the seed and let the node float.
            let seeded = (old - self.seeds[i]).abs() <= 1e-12;
            if seeded && v < old {
                continue;
            }
            // Rule 4: the convergence veto, with its tolerance so
            // post-limiting Newton jitter does not veto forever.
            if (vnew[i] - v).abs() > 1e-6 + 1e-4 * vnew[i].abs() {
                self.active_slots[i] = true;
                active = true;
            }
            self.vold[i] = v;
        }
        self.active = active;
        &self.vold
    }
}

/// Node voltages with a `$limit` junction branch replaced by its limited
/// value `vlim` — the linearization point for the Norton transform when
/// voltage limiting is active. Rule 5: displaces the minus node when it is
/// a real node (keeping a shared plus node — e.g. a BJT's internal base —
/// fixed); falls back to the plus node only when the minus is ground.
///
/// `plus`/`minus` are indices into `volts`, `None` meaning ground. `vnew`
/// is the branch's raw (unlimited) voltage, used only to recover the
/// branch's polarity relative to `vlim`.
pub fn limited_volts(volts: &[f64], plus: Option<usize>, minus: Option<usize>, vlim: f64, vnew: f64) -> Vec<f64> {
    let vp = plus.map_or(0.0, |p| volts[p]);
    let vm = minus.map_or(0.0, |m| volts[m]);
    let vbr_raw = vp - vm;
    let ty = if vbr_raw.abs() > 1e-12 { (vnew / vbr_raw).signum() } else { 1.0 };
    let vbr_eff = vlim * ty;
    let mut veff = volts.to_vec();
    if let Some(m) = minus {
        veff[m] = vp - vbr_eff;
    } else if let Some(p) = plus {
        veff[p] = vm + vbr_eff;
    }
    veff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pnjlim_passes_through_within_tolerance() {
        // A small step (|dv| <= 2*vte) is not limited at all.
        assert_eq!(pnjlim(0.5, 0.5, 0.026, 0.7), 0.5);
    }

    #[test]
    fn pnjlim_clamps_a_large_positive_step() {
        let limited = pnjlim(2.0, 0.5, 0.026, 0.7);
        assert!(limited < 2.0, "a large step past vcrit is clamped");
        assert!(limited.is_finite());
    }

    #[test]
    fn fetlim_and_limvds_stay_finite_across_the_domain() {
        for vold in [-1.0, 0.0, 1.0, 3.0, 5.0] {
            for vnew in [-2.0, 0.0, 2.0, 4.0, 8.0] {
                assert!(fetlim(vnew, vold, 1.0).is_finite());
                assert!(limvds(vnew, vold).is_finite());
            }
        }
    }

    #[test]
    fn slots_seed_with_vcrit_never_zero() {
        let limiter = Limiter::new(vec![0.7, 1.1]);
        assert_eq!(limiter.limited(), &[0.7, 1.1]);
    }

    #[test]
    fn seed_preservation_skips_a_lower_update_while_still_seeded() {
        let mut limiter = Limiter::new(vec![0.7]);
        // Raw branch voltage of 0.0 with pnjlim would pull the limited
        // value below the vcrit seed on an unbiased junction — the update
        // must be skipped so the slot stays at its seed.
        let kinds = [LimitKind::Pnjlim { vte: 0.026 }];
        limiter.update(&[0.0], &kinds);
        assert_eq!(limiter.limited(), &[0.7], "seed preserved: v < old while still seeded");
    }

    #[test]
    fn each_iteration_limits_against_the_previous_limited_value() {
        let mut limiter = Limiter::new(vec![0.7]);
        let kinds = [LimitKind::Pnjlim { vte: 0.026 }];
        limiter.update(&[2.0], &kinds);
        let after_first = limiter.limited()[0];
        limiter.update(&[after_first + 0.001], &kinds);
        let after_second = limiter.limited()[0];
        // The second iteration's clamp is anchored at `after_first`, not
        // at the raw input again — a small step from the previous limited
        // value should not re-trigger a large clamp.
        assert!((after_second - after_first).abs() < 0.01);
    }

    #[test]
    fn convergence_veto_fires_while_clamping_and_releases_within_tolerance() {
        let mut limiter = Limiter::new(vec![0.7]);
        let kinds = [LimitKind::Pnjlim { vte: 0.026 }];
        limiter.update(&[2.0], &kinds);
        assert!(limiter.active(), "a large step still clamping vetoes convergence");
        assert_eq!(limiter.active_slot(), Some(0));

        // Feed the limiter its own limited value back: vnew == vold means
        // no further clamping, so the veto must release.
        let settled = limiter.limited()[0];
        limiter.update(&[settled], &kinds);
        assert!(!limiter.active(), "no clamping once vnew matches the limited value: veto releases");
    }

    #[test]
    fn move_the_minus_node_keeps_a_shared_plus_node_fixed() {
        let volts = [1.0, 0.4]; // plus=0 (shared base), minus=1
        let veff = limited_volts(&volts, Some(0), Some(1), 0.5, 0.6);
        assert_eq!(veff[0], volts[0], "plus (shared) node untouched");
        assert_ne!(veff[1], volts[1], "minus node moved to realize vlim");
        assert_eq!(veff[1], volts[0] - 0.5, "vp - vlim (same-sign branch)");
    }

    #[test]
    fn move_the_plus_node_when_minus_is_ground() {
        let volts = [1.0];
        let veff = limited_volts(&volts, Some(0), None, 0.5, 0.6);
        assert_eq!(veff[0], 0.0 + 0.5, "vm(=0) + vlim, plus moved since minus is ground");
    }
}
