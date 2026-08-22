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

/// LRM §4.5.13 `limexp()`: exponential with its **output** growth limited
/// iteration-to-iteration. The LRM's own derivation note (Annex worked
/// example commentary) is explicit that this is `pnjlim` applied to the
/// exponential's argument: "`pnjlim` is intended for limiting arguments to
/// exponentials, and the `limexp()` function ... may be implemented through
/// a function derived from `pnjlim`."
///
/// Unlike `pnjlim`, `limexp` has no convergence-veto hook here (`$limit`'s
/// `active()` is consulted directly by the solver's Newton loop; this
/// crate has no equivalent for `limexp`) — a window that keeps clamping
/// past the first iteration can therefore let Newton declare "converged"
/// against a value the window is still actively limiting, silently
/// returning the clamped operating point instead of the true one (found
/// during development: several ngspice-oracle diode/BJT DC goldens moved
/// off their reference value with a persistent e²-per-iteration window).
/// The window below therefore applies **only to the seed iteration** —
/// `yold < 0.0`, the "no prior iteration yet" sentinel `AnalogInstance::new`
/// (`piperine-codegen`) seeds every `limexp` state slot with (`-1.0`,
/// deliberately not the state bank's default zero-init: `exp` legitimately
/// underflows to exactly `0.0` in reverse bias, and a real converged `0.0`
/// must not be misread as "still unseeded"). Once truly seeded (`yold >=
/// 0.0`), every later call passes `ynew` straight through — matching the
/// pre-fix `exp(min(x, 80))` for the entire rest of the walk — because only
/// the very first Newton iteration can carry an arbitrarily bad initial
/// guess with no iteration history yet to judge it against.
pub fn limexp_limit(ynew: f64, yold: f64) -> f64 {
    const SEED_CEILING: f64 = 1.0e12;
    if yold < 0.0 {
        return ynew.clamp(0.0, SEED_CEILING);
    }
    ynew.max(0.0)
}

/// `limexp(x)`, whole: argument clamp at 80 (matches the pre-fix bare
/// `exp(min(x, 80))` — keeps `exp` itself finite before any iteration
/// history exists to limit against) + `exp` + [`limexp_limit`]'s
/// iteration-history bound, in one call.
///
/// **Residual only.** The Jacobian must never call this: `limexp`'s
/// derivative is hard-coded transparent-to-state in
/// `piperine-codegen`'s `resolve/diff.rs` (`d_math`'s `"limexp"` arm,
/// mirroring the `$limit` case just above it) — the coefficient is always
/// the raw, state-independent `exp(x)`, never routed through this
/// function's `yold`-dependent clamp. Composing the Jacobian coefficient
/// from a `limexp`-recursing (state-reading) expression was the actual
/// root cause of a real, reproducible non-convergence found during
/// development: it corrupted the Jacobian row exactly in the iterations
/// where limiting was active, producing a singular/near-singular Newton
/// step (not a wrong DC value — genuine non-convergence). `$limit`'s own
/// `pnjlim`/`fetlim` clamp has the identical rule for the identical
/// reason (`diff.rs`'s `"limit"` `SystemFunctionCall` arm).
pub fn limexp_full(x: f64, yold: f64) -> f64 {
    limexp_limit(x.min(80.0).exp(), yold)
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

    /// The per-slot critical-voltage seeds (rule 1) — for a caller that
    /// checkpoints its own copy alongside this type's `active` flag (ABI-04:
    /// codegen's device-side wrapper packs `[active, seeds…]`, leaving
    /// `vold` in its own externally-owned state bank).
    pub fn seeds(&self) -> &[f64] {
        &self.seeds
    }

    /// Restore `seeds` and the aggregate `active` flag from a checkpoint
    /// (ABI-04). `active_slots` is intentionally not restored — it is
    /// transient, fully recomputed by the next `update`/
    /// [`advance_precomputed`](Self::advance_precomputed) call. `vold` is
    /// untouched: an externally-owned-`vold` caller (codegen) restores it
    /// separately into its own state, never through this type.
    pub fn restore_seeds(&mut self, seeds: Vec<f64>, active: bool) {
        self.active_slots = vec![false; seeds.len()];
        self.seeds = seeds;
        self.active = active;
    }

    /// Advance every slot given this iteration's raw branch voltages
    /// `vnew` (rules 2–4), storing the limited value for the next
    /// iteration to limit against (rule 3). Returns the limited values.
    /// Computes the formula itself (`kinds[i].limit`) — for a caller with no
    /// JIT of its own (e.g. a host device). A caller whose `vlim` is already
    /// computed elsewhere (codegen's JIT, since T4/T5) wants
    /// [`advance_precomputed`](Self::advance_precomputed) instead, which
    /// applies the same rules 2+4 without recomputing the formula.
    pub fn update(&mut self, vnew: &[f64], kinds: &[LimitKind]) -> &[f64] {
        let vlim: Vec<f64> = (0..self.vold.len())
            .map(|i| kinds[i].limit(vnew[i], self.vold[i], self.seeds[i]))
            .collect();
        let mut vold = std::mem::take(&mut self.vold);
        self.advance_precomputed(&vlim, vnew, &mut vold);
        self.vold = vold;
        &self.vold
    }

    /// Advance every slot given an **already-computed** limited value
    /// `vlim` (rules 2 and 4 only — rule 3's "limit against the previous
    /// limited value" and the formula dispatch itself are the caller's
    /// job, e.g. a JIT that already calls this module's own `pnjlim`/
    /// `fetlim`/`limvds`, per-slot, against `vold`). `vnew` is the raw
    /// (unlimited) value, used only for the convergence-veto comparison.
    /// `vold` is caller-owned state (codegen's shared analog state bank,
    /// checkpointed separately) — this method does not keep its own copy,
    /// only writes the new limited values back into it and updates
    /// `active`/`active_slots` (this type's own bookkeeping, `seeds` stays
    /// here too).
    pub fn advance_precomputed(&mut self, vlim: &[f64], vnew: &[f64], vold: &mut [f64]) {
        for slot in self.active_slots.iter_mut() {
            *slot = false;
        }
        let mut active = false;
        for i in 0..vold.len() {
            let old = vold[i];
            let v = vlim[i];
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
            vold[i] = v;
        }
        self.active = active;
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
    //! LRM-COVERS: 4.5.13

    use super::*;

    #[test]
    fn limexp_limit_seeds_the_first_iteration_instead_of_passing_through() {
        // yold < 0.0 (the -1.0 sentinel) means no prior iteration exists
        // yet — an overflow-scale first guess is still clamped.
        let limited = limexp_limit(1.0e30, -1.0);
        assert!(limited < 1.0e30 && limited.is_finite(), "first iteration must be limited: {limited}");
    }

    #[test]
    fn limexp_limit_treats_a_legitimate_zero_as_seeded_not_unseeded() {
        // yold == 0.0 is a REAL converged value (exp() underflow in
        // reverse bias), not the seed sentinel — it must not re-trigger
        // seed-scale clamping the way `yold <= 0.0` would.
        assert_eq!(limexp_limit(2.0, 0.0), 2.0);
    }

    #[test]
    fn limexp_limit_passes_through_every_call_once_seeded() {
        // Once `yold` is a real prior value (not the seed sentinel),
        // `limexp` passes `ynew` straight through unconditionally — even a
        // change spanning many orders of magnitude (legitimate: a diode
        // swinging from reverse bias toward forward turn-on). A window
        // that kept clamping past the first iteration let Newton declare
        // "converged" against the clamped value instead of the true one —
        // a real bug found during development (several ngspice-oracle
        // diode/BJT DC goldens moved off their reference value).
        assert_eq!(limexp_limit(2.0, 1.0), 2.0);
        assert_eq!(limexp_limit(1.0e20, 1.0), 1.0e20);
    }

    #[test]
    fn limexp_full_matches_pre_fix_behavior_when_unclamped() {
        // Within the window, limexp_full(x, yold) == exp(min(x, 80)),
        // exactly the pre-fix bare wrapper's numeric behavior.
        let x = 1.0;
        assert_eq!(limexp_full(x, 1.0), x.exp());
    }

    #[test]
    fn limexp_full_stays_finite_for_an_extreme_argument() {
        // The property that motivates the function: an argument so large
        // exp() would overflow to infinity stays finite through the
        // argument clamp, on the very first (seed) call.
        let limited = limexp_full(1000.0, -1.0);
        assert!(limited.is_finite(), "limexp_full(1000, seed) = {limited}");
    }

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

    // ── advance_precomputed: rules 2+4 over an externally-owned vold ───────
    // (codegen's device-side Limiter, since T4/T5's JIT already computes
    // vlim itself — see device/analog/limits.rs)

    #[test]
    fn advance_precomputed_preserves_the_seed_while_still_seeded() {
        let mut limiter = Limiter::new(vec![0.7]);
        let mut vold = [0.7];
        // A precomputed vlim below the seed on an unbiased junction must be
        // skipped (rule 2), same as `update`'s internal formula path.
        limiter.advance_precomputed(&[0.5], &[0.0], &mut vold);
        assert_eq!(vold, [0.7], "seed preserved: v < old while still seeded");
    }

    #[test]
    fn advance_precomputed_veto_fires_while_clamping_and_releases_within_tolerance() {
        let mut limiter = Limiter::new(vec![0.7]);
        let mut vold = [0.7];
        // A precomputed vlim far from vnew is still clamping: vetoes convergence.
        limiter.advance_precomputed(&[1.5], &[2.0], &mut vold);
        assert!(limiter.active(), "vlim far from vnew vetoes convergence");
        assert_eq!(limiter.active_slot(), Some(0));
        assert_eq!(vold, [1.5], "the caller's vold slice advances to the precomputed vlim");

        // Feed vnew == vlim back: no further clamping, so the veto releases.
        limiter.advance_precomputed(&[1.5], &[1.5], &mut vold);
        assert!(!limiter.active(), "no clamping once vnew matches vlim: veto releases");
    }

    #[test]
    fn seeds_reports_the_construction_time_critical_voltages() {
        let limiter = Limiter::new(vec![0.7, 1.1]);
        assert_eq!(limiter.seeds(), &[0.7, 1.1]);
    }

    #[test]
    fn restore_seeds_reinstates_seeds_and_active_but_not_vold() {
        let mut limiter = Limiter::new(vec![0.7]);
        let mut vold = [0.7];
        limiter.advance_precomputed(&[1.5], &[2.0], &mut vold);
        assert!(limiter.active());
        assert_eq!(limiter.seeds(), &[0.7]);

        // Restoring different seeds/active (as a checkpoint round-trip would)
        // takes effect immediately...
        limiter.restore_seeds(vec![0.3], false);
        assert_eq!(limiter.seeds(), &[0.3]);
        assert!(!limiter.active());
        // ...and does not touch the internal vold cache — it was never
        // written by `advance_precomputed` in the first place (an
        // externally-owned-vold caller never reads it through this type),
        // so it is still `new`'s original seed.
        assert_eq!(limiter.limited(), &[0.7]);
    }

    #[test]
    fn advance_precomputed_does_not_keep_its_own_vold_copy() {
        let mut limiter = Limiter::new(vec![0.0]);
        let mut vold = [3.0]; // caller's own state, already advanced elsewhere
        limiter.advance_precomputed(&[3.0], &[3.0], &mut vold);
        assert_eq!(vold, [3.0]);
        // `limited()` (the internal `vold` set by `new`/`update`) is
        // untouched — proving `advance_precomputed` never wrote to
        // `self.vold`, only to the caller-supplied slice.
        assert_eq!(
            limiter.limited(), &[0.0],
            "advance_precomputed must not mutate the internal vold cache"
        );
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
