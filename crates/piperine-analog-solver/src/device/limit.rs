//! `LIMIT` inputs (abi.md §4.2, §4.3, A2, A3; design `LIMIT`). Per limit the
//! assembly holds the iterate `x̂` (`VALUE`), the proposed `x`, `LAST` and
//! `AGE`. In each evaluation the functions writing limits run first, from
//! `LAST` and `AGE`; `x̂` becomes the `LIMITED` they wrote and `x` the
//! `PROPOSED`. The readers of `VALUE` then report plain values at `x̂`, and a
//! reader's term is linearized as `v(x̂) + g·(x − x̂)` with Jacobian `g·p`,
//! `g` its partials against the limit and `p` those of `PROPOSED` against the
//! pins. A limit no function writes is answered `LIMITED = PROPOSED = LAST`
//! (A3). While any limit has `|x̂ − x| > 1e-6 + 1e-4·|x|` the solve is not
//! converged (Appendix C.2). A function flagged `OWNS_CORRECTION` declares no
//! partial against the limits it writes, so its terms are stamped as written
//! and only the veto applies.

use piperine_abi::raw::input;

use super::assembly::{Held, InstanceState};
use super::flow::Linear;
use super::plan::{InstancePlan, TermPlan};

/// The limiting tolerance: absolute part.
const LIMIT_ABSTOL: f64 = 1e-6;
/// The limiting tolerance: relative part.
const LIMIT_RELTOL: f64 = 1e-4;

/// One limit whose held iterate differs from its proposed value beyond the
/// limiting tolerance: it vetoes convergence.
#[derive(Clone, Debug, PartialEq)]
pub struct LimitState {
    /// The instance's path.
    pub instance: String,
    /// The input's name.
    pub input: String,
    /// `x`, the proposed value.
    pub proposed: f64,
    /// `x̂`, the held iterate.
    pub limited: f64,
}

impl LimitState {
    /// Whether `held` is outside the limiting tolerance of `proposed`.
    pub fn is_active(held: f64, proposed: f64) -> bool {
        (held - proposed).abs() > LIMIT_ABSTOL + LIMIT_RELTOL * proposed.abs()
    }
}

/// The limits of one instance: the part of the plan every limit operation reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits<'a> {
    /// The instance whose limits these are.
    instance: &'a InstancePlan,
}

impl<'a> Limits<'a> {
    /// The limits of `instance`.
    pub fn new(instance: &'a InstancePlan) -> Self {
        Self { instance }
    }

    /// Sets each limit's `x̂` and `x` from the functions that just wrote them,
    /// or to `LAST` for a limit no function writes (A3). In a small-signal sweep
    /// `x̂ = x` (abi.md §4.3).
    pub fn take_iterates(&self, st: &mut InstanceState, small_signal: bool) {
        for (i, x) in self.instance.inputs.iter().enumerate() {
            if x.kind != input::LIMIT {
                continue;
            }
            let last = st.held[i].last;
            let read = |at: Option<(usize, usize)>| at.map_or(last, |(f, w)| st.fns[f].f64(w));
            let (limited, proposed) = (read(x.limited), read(x.proposed));
            let h = &mut st.held[i];
            h.value = if small_signal { proposed } else { limited };
            h.proposed = proposed;
        }
    }

    /// Term `t` at the iterate: its value and direct partials, plus, for each
    /// partial `g` against a limit, `g·(x − x̂)` on the value and `g·p_j` on the
    /// columns (read through the [`Linear`] when asked for).
    pub fn linear<'s>(&self, t: &'s TermPlan, st: &'s InstanceState) -> Linear<'s>
    where
        'a: 's,
    {
        let Some((fi, w)) = t.value else { return Linear::none() };
        let mut v = st.fns[fi].f64(w);
        for (p, i) in &t.via {
            let x = &self.instance.inputs[*i];
            if x.kind != input::LIMIT {
                continue;
            }
            let g = st.fns[fi].jac[*p];
            let Held { value, proposed, .. } = st.held[*i];
            v += g * (proposed - value);
        }
        Linear::of(v, t, self.instance, st)
    }

    /// The limits of the instance that veto convergence.
    pub fn vetoes(&self, st: &InstanceState, out: &mut Vec<LimitState>) {
        for (x, h) in self.instance.inputs.iter().zip(&st.held) {
            if x.kind == input::LIMIT && LimitState::is_active(h.value, h.proposed) {
                out.push(LimitState {
                    instance: self.instance.path.clone(),
                    input: x.name.clone(),
                    proposed: h.proposed,
                    limited: h.value,
                });
            }
        }
    }

    /// Advances every written limit's `LAST` to its `x̂` with `AGE = age`.
    pub fn advance(&self, st: &mut InstanceState, age: u8) {
        for (x, h) in self.instance.inputs.iter().zip(st.held.iter_mut()) {
            if x.kind == input::LIMIT && x.limited.is_some() {
                h.last = h.value;
                h.age = age;
            }
        }
    }
}
