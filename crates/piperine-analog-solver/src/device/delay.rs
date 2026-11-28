//! `DELAY` terms (abi.md §4.3, design `DELAY`): each keeps the `(t, v)` of
//! its accepted solutions, pushed by `accept`, pruned to what a delay up to
//! its `max` can reach. A transient stage stamps the value at `t − td`,
//! linearly interpolated from that history; a point inside the current step
//! (`t − td` after the last accepted time) interpolates between the last
//! accepted value and the current one, whose partials then carry the weight
//! of the current end. An equilibrium solve stamps the value itself. A `td`
//! beyond `max` fails naming instance and term.

use crate::error::Error;
use crate::math::linear::Stamp;
use crate::netlist::AnalogReference;

use super::flow::{FlowStamp, Linear};
use super::plan::TermPlan;

/// The accepted `(t, v)` of one `DELAY` term, in time order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Delayed {
    points: Vec<(f64, f64)>,
}

impl Delayed {
    /// Records the value at an accepted time and drops what no delay up to
    /// `max` can read any more (one point before `t − max` stays, for the
    /// interpolation). A value at the time of the last one (a zero-width solve, which replaces
    /// the solution there) replaces it.
    pub fn push(&mut self, t: f64, v: f64, max: f64) {
        match self.points.last_mut() {
            Some(last) if last.0 == t => last.1 = v,
            _ => self.points.push((t, v)),
        }
        let horizon = t - max;
        let before = self.points.iter().take_while(|(s, _)| *s < horizon).count();
        if before > 1 {
            self.points.drain(..before - 1);
        }
    }

    /// The value at `s` as `(h, w)`: the history's part `h` and the weight
    /// `w` of the current value at time `now` (0 when `s` lies in the
    /// accepted history).
    fn at(&self, s: f64, now: f64) -> (f64, f64) {
        let Some(&(tl, vl)) = self.points.last() else { return (0.0, 1.0) };
        if s >= now {
            return (0.0, 1.0);
        }
        if s >= tl {
            if now <= tl {
                return (vl, 0.0);
            }
            let w = (s - tl) / (now - tl);
            return ((1.0 - w) * vl, w);
        }
        let (t0, v0) = self.points[0];
        if s <= t0 {
            return (v0, 0.0);
        }
        let i = self.points.partition_point(|(t, _)| *t <= s);
        let ((ta, va), (tb, vb)) = (self.points[i - 1], self.points[i]);
        (va + (vb - va) * (s - ta) / (tb - ta), 0.0)
    }
}

/// The stamps of one `DELAY` term in a transient stage: the part of the assembly it reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DelayStamp<'a> {
    /// The path of the instance, for the error.
    pub path: &'a str,
    /// The term whose rows are stamped.
    pub term: &'a TermPlan,
    /// The term at the iterate, its current value.
    pub linear: Linear<'a>,
    /// The accepted values of the term.
    pub history: &'a Delayed,
    /// The time of the solve.
    pub now: f64,
    /// The delay.
    pub delay: f64,
    /// The bound of the delay.
    pub max: f64,
}

impl DelayStamp<'_> {
    /// The stamps of a `DELAY` term of current value `linear` at time `now` with
    /// delay `td` and bound `max`, in a transient stage.
    pub fn stamp(&self, x: &[f64], out: &mut Vec<Stamp<AnalogReference, f64>>) -> Result<(), Error> {
        if self.delay > self.max {
            return Err(Error::device(format!(
                "instance `{}`: term `{}` ({}): delay {} exceeds its max {}",
                self.path, self.term.name, self.term.origin, self.delay, self.max
            )));
        }
        let (h, w) = self.history.at(self.now - self.delay, self.now);
        let at = self.linear.scaled(h + w * self.linear.v, w);
        FlowStamp::new(self.term, at).norton(x, out);
        Ok(())
    }
}
