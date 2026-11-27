//! `FLOW` terms (abi.md A1, design `FLOW`): a term `{plus, minus}` of value
//! `v` with partials `g_j` against continuous pins `j` adds the Norton form
//! of its linearization at the iterate: `g_j` at (`plus`, `j`), `−g_j` at
//! (`minus`, `j`), `−(v − Σ g_j·x_j)` to the right side of `plus` and its
//! negative to `minus`. A `GROUND` side has no row.

use crate::math::linear::Stamp;
use crate::netlist::AnalogReference;

use piperine_abi::raw::input;

use super::assembly::InstanceState;
use super::plan::{InstancePlan, TermPlan};

/// A term at the iterate: its value, and a view over its partials against continuous pins (one
/// per declared partial, zeros included, so the positions are the same in every solve). The
/// partials are read from the writer's last call when asked for, so a term costs no allocation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Linear<'a> {
    pub v: f64,
    /// The plan and state the partials are read from; `None` for a term nothing writes.
    source: Option<Source<'a>>,
    /// The factor every partial is scaled by (1 unless the term is a `DELAY`'s).
    weight: f64,
}

/// Where a [`Linear`]'s partials live.
#[derive(Clone, Copy, Debug)]
struct Source<'a> {
    term: &'a TermPlan,
    instance: &'a InstancePlan,
    state: &'a InstanceState,
}

impl<'a> Linear<'a> {
    /// A term with no value: no partials.
    pub fn none() -> Self {
        Self { v: 0.0, source: None, weight: 1.0 }
    }

    /// Term `term` of `instance` with value `v`, its partials read from `state`.
    pub fn of(v: f64, term: &'a TermPlan, instance: &'a InstancePlan, state: &'a InstanceState) -> Self {
        Self { v, source: Some(Source { term, instance, state }), weight: 1.0 }
    }

    /// This term with value `v` and every partial multiplied by `weight`.
    pub fn scaled(&self, v: f64, weight: f64) -> Self {
        Self { v, source: self.source, weight: self.weight * weight }
    }

    /// The partials as `(column, g)`: the direct ones in declaration order, then, for each
    /// limit the value reads, the limit's own partials against the unknowns scaled by the
    /// value's partial against the limit.
    pub fn cols(&self) -> impl Iterator<Item = (&'a AnalogReference, f64)> + use<'a> {
        let weight = self.weight;
        let written = self.source.into_iter().filter_map(|source| source.term.value.map(|(writer, _)| (source, writer)));
        written.flat_map(move |(Source { term, instance, state }, writer)| {
            let jac = &state.fns[writer].jac;
            let direct = term.direct.iter().map(move |(p, column)| (column, weight * jac[*p]));
            let through = term.via.iter().flat_map(move |(p, input)| {
                let x = &instance.inputs[*input];
                let proposer = x.proposed.filter(|_| x.kind == input::LIMIT);
                proposer.into_iter().flat_map(move |(proposer, _)| {
                    let g = jac[*p];
                    x.p.iter().map(move |(pp, column)| (column, weight * (g * state.fns[proposer].jac[*pp])))
                })
            });
            direct.chain(through)
        })
    }

    /// `Σ g_j·x_j` over the iterate. `piperine-interp`'s affine rule (INT-12) writes a linear row as `c + this fold`; its test `an_affine_term_is_constant_plus_folded_products` fails if the fold order changes.
    pub fn dot(&self, x: &[f64]) -> f64 {
        self.cols().fold(0.0, |s, (c, g)| c.idx().map_or(s, |unknown| s + g * x[unknown]))
    }
}

/// Adds `weight·g_j` at (`plus`, `j`) and its negative at (`minus`, `j`),
/// and `rhs` to the right side of `plus` and its negative to `minus`.
pub(crate) fn stamp_rows(
    t: &TermPlan,
    lin: &Linear<'_>,
    weight: f64,
    rhs: f64,
    out: &mut Vec<Stamp<AnalogReference, f64>>,
) {
    for (c, g) in lin.cols() {
        let g = weight * g;
        if let Some(p) = &t.plus {
            out.push(Stamp::Matrix(p.clone(), c.clone(), g));
        }
        if let Some(m) = &t.minus {
            out.push(Stamp::Matrix(m.clone(), c.clone(), -g));
        }
    }
    if let Some(p) = &t.plus {
        out.push(Stamp::Rhs(p.clone(), rhs));
    }
    if let Some(m) = &t.minus {
        out.push(Stamp::Rhs(m.clone(), -rhs));
    }
}

/// The Norton stamps of a `FLOW` term at iterate `x`.
pub(crate) fn stamp_flow(t: &TermPlan, lin: &Linear<'_>, x: &[f64], out: &mut Vec<Stamp<AnalogReference, f64>>) {
    stamp_rows(t, lin, 1.0, -(lin.v - lin.dot(x)), out);
}
