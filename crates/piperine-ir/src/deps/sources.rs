//! The abstract values of the walk: the sources a value reads and its class on the lattice.

use std::collections::BTreeSet;

use super::Deps;
use crate::model::Env;

/// What a value reads: the node potentials, params, environment, sites and carriers it depends on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(super) struct Sources {
    pub(super) nodes: BTreeSet<u32>,
    pub(super) params: BTreeSet<u32>,
    pub(super) env: BTreeSet<Env>,
    pub(super) limits: BTreeSet<u32>,
    pub(super) stims: BTreeSet<u32>,
    pub(super) carriers: BTreeSet<u32>,
    /// The branches whose flow is read (`Expr::Flow`).
    pub(super) flows: BTreeSet<u32>,
    /// The carriers whose previous value is read (`Expr::Previous`).
    pub(super) previous: BTreeSet<u32>,
    /// The resolved carriers whose `others()` is read (`Expr::Others`).
    pub(super) others: BTreeSet<u32>,
    /// The carriers read as the drives of an `Expr::Resolve`.
    pub(super) resolve_drives: BTreeSet<u32>,
    /// The events whose `CHANGED` byte this microstep is read (`Expr::Fired`).
    pub(super) fired: BTreeSet<u32>,
    /// The derivative sites whose rate is read (`Expr::Derivative`).
    pub(super) derivatives: BTreeSet<u32>,
    /// The delay sites whose output is read (`Expr::Delay`).
    pub(super) delays: BTreeSet<u32>,
    /// The noise sources whose value is read (`Expr::Noise`).
    pub(super) noises: BTreeSet<u32>,
    /// The session parameters whose value is read (`Expr::SessionParameter`).
    pub(super) session_parameters: BTreeSet<u32>,
    /// The params whose previous value is read (`Expr::ParamPrevious`).
    pub(super) param_previous: BTreeSet<u32>,
    /// The constant elements of an array carrier whose value is read (`mem[3]`), as `(carrier, element)`.
    pub(super) elements: BTreeSet<(u32, u64)>,
    /// The constant elements whose previous value is read (`mem[3].prev()`).
    pub(super) element_previous: BTreeSet<(u32, u64)>,
    /// The constant elements whose `changed` byte is read (`mem[3].changed()`).
    pub(super) element_changed: BTreeSet<(u32, u64)>,
    /// The array carriers whose `changed` byte is read at a run-time element (`mem[i].changed()`).
    pub(super) whole_changed: BTreeSet<u32>,
}

impl Sources {
    /// Adds everything `other` reads to this set.
    pub(super) fn join(&mut self, other: &Sources) {
        self.nodes.extend(&other.nodes);
        self.params.extend(&other.params);
        self.env.extend(&other.env);
        self.limits.extend(&other.limits);
        self.stims.extend(&other.stims);
        self.carriers.extend(&other.carriers);
        self.flows.extend(&other.flows);
        self.previous.extend(&other.previous);
        self.others.extend(&other.others);
        self.resolve_drives.extend(&other.resolve_drives);
        self.fired.extend(&other.fired);
        self.derivatives.extend(&other.derivatives);
        self.delays.extend(&other.delays);
        self.noises.extend(&other.noises);
        self.session_parameters.extend(&other.session_parameters);
        self.param_previous.extend(&other.param_previous);
        self.elements.extend(&other.elements);
        self.element_previous.extend(&other.element_previous);
        self.element_changed.extend(&other.element_changed);
        self.whole_changed.extend(&other.whole_changed);
    }

    /// The sorted public form of this set.
    pub(super) fn to_deps(&self) -> Deps {
        Deps {
            nodes: self.nodes.iter().copied().collect(),
            params: self.params.iter().copied().collect(),
            env: self.env.iter().copied().collect(),
            limits: self.limits.iter().copied().collect(),
            stims: self.stims.iter().copied().collect(),
            delays: self.delays.iter().copied().collect(),
            noise_sources: self.noises.iter().copied().collect(),
            session_parameters: self.session_parameters.iter().copied().collect(),
        }
    }
}

/// How a value depends on the node potentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum Class {
    Constant,
    Affine,
    Nonlinear,
}

/// The abstract value of an expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Value {
    pub(super) sources: Sources,
    pub(super) class: Class,
}

impl Value {
    /// A value that reads nothing and is `Constant`.
    pub(super) fn constant() -> Self {
        Value { sources: Sources::default(), class: Class::Constant }
    }

    /// A value that reads `sources`, of class `class`.
    pub(super) fn of(sources: Sources, class: Class) -> Self {
        Value { sources, class }
    }

    /// The sources of every value; the class is `Constant` when every class is, `Nonlinear`
    /// otherwise: the rule of every operator that is not linear.
    pub(super) fn nonlinear<'v>(values: impl IntoIterator<Item = &'v Value>) -> Self {
        let mut out = Value::constant();
        for v in values {
            out.sources.join(&v.sources);
            if v.class != Class::Constant {
                out.class = Class::Nonlinear;
            }
        }
        out
    }
}

impl Value {
    /// The least value above `self` and `other`: the sources of both, the larger class.
    pub(super) fn join(&mut self, other: &Value) {
        self.sources.join(&other.sources);
        self.class = self.class.max(other.class);
    }

    /// `self`, read under `conditions`: it also reads what they read, and is no longer affine
    /// when one is not `Constant`, as a contribution under an `if` is not (a switch).
    pub(super) fn under(&mut self, conditions: &[Value]) {
        for condition in conditions {
            self.sources.join(&condition.sources);
            if condition.class != Class::Constant {
                self.class = Class::Nonlinear;
            }
        }
    }
}
