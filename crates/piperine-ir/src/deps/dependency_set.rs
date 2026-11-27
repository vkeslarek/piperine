//! The answer of a dependency query: the sorted sources a target reads.

use super::SiteIndex;
use crate::model::Env;

/// A dependency set. Fields come in the fixed order nodes, params, environment reads, `limit`
/// sites, `ac_stim` sites; each is sorted by index (environment reads in `Env` order) and holds
/// no duplicate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deps {
    /// Body nodes probed, by index (`Body::nodes`); ground is not a node.
    pub nodes: Vec<u32>,
    /// Params read, by index (`Body::params`).
    pub params: Vec<u32>,
    pub env: Vec<Env>,
    pub limits: Vec<SiteIndex>,
    pub stims: Vec<SiteIndex>,
    /// Delay sites whose output (`Expr::Delay`) is read, by index (`Body::delays`).
    pub delays: Vec<u32>,
    /// Noise sources whose value (`Expr::Noise`) is read, by index (`Body::noise_sources`).
    pub noise_sources: Vec<u32>,
    /// Session parameters read (`Expr::SessionParameter`), by index (`Body::session_parameters`).
    pub session_parameters: Vec<u32>,
}
