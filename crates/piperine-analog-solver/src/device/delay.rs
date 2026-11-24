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

use super::device_error;
use super::flow::{Linear, stamp_flow};
use super::plan::TermPlan;

/// The accepted `(t, v)` of one `DELAY` term, in time order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Delayed {
    points: Vec<(f64, f64)>,
}

impl Delayed {
    /// Records the value at an accepted time and drops what no delay up to
    /// `max` can read any more (one point before `t − max` stays, for the
    /// interpolation).
    pub fn push(&mut self, t: f64, v: f64, max: f64) {
        self.points.push((t, v));
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

/// The stamps of a `DELAY` term of current value `lin` at time `now` with
/// delay `td` and bound `max`, in a transient stage.
pub(crate) fn stamp_delay(
    path: &str,
    t: &TermPlan,
    lin: &Linear<'_>,
    hist: &Delayed,
    (now, td, max): (f64, f64, f64),
    x: &[f64],
    out: &mut Vec<Stamp<AnalogReference, f64>>,
) -> Result<(), Error> {
    if td > max {
        return Err(device_error(format!(
            "instance `{path}`: term `{}` ({}): delay {td} exceeds its max {max}",
            t.name, t.origin
        )));
    }
    let (h, w) = hist.at(now - td, now);
    let at = lin.scaled(h + w * lin.v, w);
    stamp_flow(t, &at, x, out);
    Ok(())
}
