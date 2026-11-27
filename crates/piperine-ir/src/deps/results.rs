//! The memo of a body's dependency walk and the result the walk gives.

use std::sync::OnceLock;

use super::Sources;

/// The memo of a body's dependency walk. It is derived data: it never takes part in equality,
/// cloning or serialization, so an IR compares, clones and round-trips the same whether or not
/// it has been asked.
#[derive(Debug, Default)]
pub(crate) struct DepsMemo(pub(super) OnceLock<BodyDeps>);

impl Clone for DepsMemo {
    fn clone(&self) -> Self {
        DepsMemo::default()
    }
}

impl PartialEq for DepsMemo {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The result of one body walk, one entry per branch, site and param.
#[derive(Debug)]
pub(crate) struct BodyDeps {
    pub(super) flow: Vec<Sources>,
    pub(super) charge: Vec<Sources>,
    pub(super) affine: Vec<bool>,
    pub(super) limits: Vec<Sources>,
    pub(super) stims: Vec<Sources>,
    pub(super) checks: Vec<Sources>,
    pub(super) drives: Vec<Sources>,
    pub(super) drive_afters: Vec<Sources>,
    pub(super) guards: Vec<Sources>,
    pub(super) crosses: Vec<Sources>,
    pub(super) ats: Vec<Sources>,
    pub(super) reactions: Vec<Sources>,
    pub(super) caps: Vec<Sources>,
    pub(super) ttols: Vec<Sources>,
    pub(super) etols: Vec<Sources>,
    pub(super) derivative_states: Vec<Sources>,
    pub(super) derivative_abstols: Vec<Sources>,
    pub(super) delay_inputs: Vec<Sources>,
    pub(super) delay_times: Vec<Sources>,
    pub(super) delay_maxima: Vec<Sources>,
    pub(super) noise_densities: Vec<Sources>,
    pub(super) traces: Vec<Sources>,
}
