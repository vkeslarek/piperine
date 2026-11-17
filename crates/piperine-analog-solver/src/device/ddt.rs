//! `DDT` terms (abi.md §4.3, design `DDT`): the solver owns the companion.
//! Each `DDT` term keeps a ring of its accepted values `q`, pushed once per
//! converged solve at the points the Newton state buffer gains a row, so
//! lookback `k` here is `states.view(k)` there. A transient stage adds the
//! TR-BDF2 companion current of `TrBdf2::stage_coeffs` over that ring, with
//! the trapezoidal stage's previous-derivative correction, exactly as the
//! hand-written capacitor of `piperine-solver/tests/helpers/devices.rs`
//! (`C::load_transient`) computes it with `c·v(row)` in place of the ring.
//! An equilibrium solve, or a step that is not positive, adds nothing.

use crate::math::integration::{TrBdf2, TrBdf2Phase};
use crate::math::linear::Stamp;
use crate::netlist::AnalogReference;

use super::flow::{Linear, stamp_rows};
use super::plan::TermPlan;

/// One TR-BDF2 stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    pub phase: TrBdf2Phase,
    /// The full step `h` of the TR-BDF2 step this stage belongs to.
    pub h: f64,
    /// The previous accepted step; 0 after a discontinuity (restart).
    pub prev_h: f64,
}

/// The accepted values of one `DDT` term, newest first: lookbacks 1 to 3, as
/// a transient Newton buffer of depth 4 holds them behind its iterate, each with the
/// companion current (the term's derivative) its stage computed there: 0 for a row no stage
/// produced (an operating point, an initial condition).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct History {
    q: [f64; 3],
    i: [f64; 3],
    len: usize,
}

impl History {
    /// Records the value of a converged solve and the companion current at it.
    pub fn push(&mut self, q: f64, i: f64) {
        self.q = [q, self.q[0], self.q[1]];
        self.i = [i, self.i[0], self.i[1]];
        self.len = (self.len + 1).min(3);
    }

    /// The companion current `lookback` (1 to 3) solves back; 0 when the history is shorter.
    pub fn current_back(&self, lookback: usize) -> f64 {
        if lookback <= self.len { self.i[lookback - 1] } else { 0.0 }
    }

    /// The value `lookback` (1 to 3) solves back; 0 when the history is
    /// shorter, as a missing buffer row reads 0 in `C::load_transient`.
    pub fn back(&self, lookback: usize) -> f64 {
        if lookback <= self.len { self.q[lookback - 1] } else { 0.0 }
    }
}

/// The companion current `i = c0·q + c1·q₁ + c2·q₂` of a term whose value at the solve of `stage`
/// is `q` (minus the previous BDF2 derivative in a trapezoidal stage after a regular step); 0
/// when the step is not positive.
pub(crate) fn companion_current(stage: Stage, q: f64, hist: &History) -> f64 {
    if stage.h <= 0.0 {
        return 0.0;
    }
    let (c0, c1, c2) = TrBdf2::stage_coeffs(stage.phase, stage.h, stage.prev_h);
    let mut i = c0 * q + c1 * hist.back(1) + c2 * hist.back(2);
    if matches!(stage.phase, TrBdf2Phase::Trapezoidal) && stage.prev_h > 0.0 {
        let (d0, d1, d2) = TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, stage.prev_h);
        i -= d0 * hist.back(1) + d1 * hist.back(2) + d2 * hist.back(3);
    }
    i
}

/// The companion stamps of a `DDT` term of value `lin.v = q` at iterate `x`:
/// `i = c0·q + c1·q₁ + c2·q₂` (minus the previous BDF2 derivative in a
/// trapezoidal stage after a regular step), the Jacobian `c0·∂q/∂x_j` and the
/// right side `c0·Σ ∂q/∂x_j·x_j − i`. With `h ≤ 0` every position is stamped
/// with 0.
pub(crate) fn stamp_ddt(
    t: &TermPlan,
    lin: &Linear,
    hist: &History,
    stage: Stage,
    x: &[f64],
    out: &mut Vec<Stamp<AnalogReference, f64>>,
) {
    if stage.h <= 0.0 {
        stamp_rows(t, lin, 0.0, 0.0, out);
        return;
    }
    let (c0, ..) = TrBdf2::stage_coeffs(stage.phase, stage.h, stage.prev_h);
    let i = companion_current(stage, lin.v, hist);
    stamp_rows(t, lin, c0, c0 * lin.dot(x) - i, out);
}
