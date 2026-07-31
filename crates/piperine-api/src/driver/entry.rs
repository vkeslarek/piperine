//! [`Driver`], [`Policy`], and [`DriverOutcome`] — the driver's own type
//! (MD-34: `mod.rs` declares, this file implements).

use crate::error::Error;
use crate::session::{Session, SolverConfig};
use piperine_solver::prelude::CheckPosture;

use super::center::{self, CenterResult, CenterSpec};
use super::high_sigma::{self, HighSigmaResult, HighSigmaSpec};
use super::optimize::{self, OptimizeResult, OptimizeSpec};

/// One of the driver's three policies (design §C4).
#[derive(Debug, Clone)]
pub enum Policy {
    /// Feasibility then objective, on gradients where available (T17/T18).
    Optimize(OptimizeSpec),
    /// Worst-case-distance centering (T19).
    Center(CenterSpec),
    /// Mean-shift importance-sampling tail estimation (T20).
    HighSigma(HighSigmaSpec),
}

/// One [`Driver::run`] call's typed result — which policy ran decides
/// which variant comes back.
#[derive(Debug, Clone)]
pub enum DriverOutcome {
    Optimize(OptimizeResult),
    Center(CenterResult),
    HighSigma(HighSigmaResult),
}

/// The one driver type (T16, DVG-14): built from a [`Policy`], shared by
/// both hosts (MD-22) because it lives in `piperine-api`.
#[derive(Debug, Clone)]
pub struct Driver {
    policy: Policy,
}

impl Driver {
    /// Build a driver for `policy`.
    pub fn new(policy: Policy) -> Self {
        Self { policy }
    }

    /// The posture this driver always solves in — [`CheckPosture::Collect`],
    /// by construction (T16 AC2), independent of any [`SolverConfig`] it is
    /// run with.
    pub fn posture(&self) -> CheckPosture {
        CheckPosture::Collect
    }

    /// Run the configured policy over `session`. Forces `collect` posture
    /// (T16 AC2) on a clone of `config` before dispatching — every other
    /// field of `config` (tolerances, `require_scope`, convergence
    /// tunables) reaches the policy unchanged.
    pub fn run(&self, session: &mut Session, config: &SolverConfig) -> Result<DriverOutcome, Error> {
        let mut collect_config = config.clone();
        collect_config.checks = CheckPosture::Collect;
        match &self.policy {
            Policy::Optimize(spec) => {
                optimize::run(session, spec, &collect_config).map(DriverOutcome::Optimize)
            }
            Policy::Center(spec) => {
                center::run(session, spec, &collect_config).map(DriverOutcome::Center)
            }
            Policy::HighSigma(spec) => {
                high_sigma::run(session, spec, &collect_config).map(DriverOutcome::HighSigma)
            }
        }
    }
}
