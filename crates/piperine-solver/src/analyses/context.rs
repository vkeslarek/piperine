//! The run configuration every analysis driver speaks: `Tolerances` (immutable
//! per-run numerics plus the convergence tests that read only them), `Policy`
//! (the mutable convergence tunables a driver owns, MD-04), and `Context` (the
//! shared immutable bundle handed to every analysis, and the owner of the
//! one-time process init, MD-06).
//!
//! Distinct from `config.rs`, which is pure data — the tunable
//! schedule/gain/trace literals with no logic beyond construction.

use crate::analog::Netlist;
use crate::analyses::config::TraceFlags;
use faer::{Par, set_global_parallelism};
use ndarray::ArrayView1;
use std::num::NonZeroUsize;
use std::sync::Once;

static INIT: Once = Once::new();

// ── Tolerances (immutable, Clone) ───────────────────────────────────────────

/// Immutable per-run numerical tolerances. Shared across every analysis
/// through `Context`. Extracted from the old flat `Context` fields (MD-04).
///
/// `Clone`, not `Copy` (as of T48/LRMW2-53): `node_vntol_overrides` is an
/// `Rc<HashMap<..>>`, needed for §6.5.8's per-node nature-`abstol`
/// inheritance ("The node shall be treated as having a potential abstol
/// with a value equal to the smallest abstol of all the potential natures
/// of all the disciplines with which it is associated") — a genuinely
/// per-net override the single flat `vntol` scalar cannot express. `Rc`
/// keeps a clone of `Tolerances` (still handed around by value at every
/// call site that used to rely on `Copy`) cheap — one pointer bump, not a
/// map copy.
#[derive(Debug, Clone)]
pub struct Tolerances {
    pub gmin: f64,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub min_res: f64,
    /// Truncation error tolerance for adaptive timestep (default: 7.0)
    pub trtol: f64,
    /// Charge tolerance in Coulombs for truncation error (default: 1e-14)
    pub chgtol: f64,
    pub temperature: f64,
    pub tnom: f64,
    /// Circuit-wide diagonal conductance to ground on every node (default 0).
    /// Helps convergence on floating/poorly-damped topologies (ngspice gshunt).
    pub gshunt: f64,
    /// Per-node potential-`abstol` override (§6.5.8, LRMW2-53, T48) — a
    /// node present here uses this value instead of `vntol` in both
    /// `has_converged`'s update test and `residual_test`'s residual
    /// check, which is what makes a tighter nature `abstol` *observably*
    /// tighten convergence rather than merely being carried and ignored.
    /// Empty by default (every node uses the flat `vntol`, unchanged
    /// behavior). A node absent here is unaffected — `None` means "no
    /// declared-nature information reached this node," not "abstol is
    /// zero."
    ///
    /// Flow-nature `abstol` inheritance (§6.5.8's other half, "a flow
    /// abstol... of all the flow natures") is not covered by this field
    /// — a disclosed scope cut, not silently dropped: LRMW2-53's
    /// Done-when asks for "a tighter nature abstol observably tightens
    /// convergence on the connected net," which the potential/node case
    /// alone already demonstrates.
    pub node_vntol_overrides: std::rc::Rc<std::collections::HashMap<crate::analog::NodeIdentifier, f64>>,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            gmin: 1e-12,
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            min_res: 1e-12,
            trtol: 7.0,
            chgtol: 1e-14,
            temperature: 300.15,
            tnom: 300.15,
            gshunt: 0.0,
            node_vntol_overrides: std::rc::Rc::new(std::collections::HashMap::new()),
        }
    }
}

impl Tolerances {
    /// §6.5.8's per-node potential-`abstol` (a tighter value than the flat
    /// `vntol`, if the node has one declared) — `self.vntol` for any node
    /// absent from `node_vntol_overrides`, unchanged from before this
    /// field existed.
    fn vntol_for(&self, variable: &crate::analog::AnalogVariable) -> f64 {
        match variable {
            crate::analog::AnalogVariable::Node(id) => {
                self.node_vntol_overrides.get(id).copied().unwrap_or(self.vntol)
            }
            _ => self.vntol,
        }
    }

    /// The convergence test that used to be `Context::has_converged` — moved
    /// here because it only reads tolerance fields. Same logic, same output.
    pub fn has_converged(
        &self,
        old_values_opt: Option<ArrayView1<f64>>,
        new_values: &ArrayView1<f64>,
        netlist: &Netlist,
    ) -> bool {
        let Some(old_values) = old_values_opt else { return false; };

        netlist
            .all_references()
            .iter()
            .filter(|s| s.idx().is_some())
            .all(|reference| {
                let index = reference.idx().unwrap();

                if index >= old_values.len() || index >= new_values.len() {
                    return true;
                }

                let old_v = old_values[index];
                let new_v = new_values[index];

                let abs_limit = if reference.is_branch() {
                    self.abstol
                } else {
                    self.vntol_for(reference.variable())
                };

                let magnitude = old_v.abs().max(new_v.abs());
                let allowed_error = self.reltol * magnitude + abs_limit;
                let diff = (new_v - old_v).abs();

                diff <= allowed_error
            })
    }

    /// ngspice `NIconvTest`: every node's current imbalance (and every branch
    /// row's equation residual) must be within tolerance.
    pub fn residual_test(
        &self,
        netlist: &Netlist,
        residual: &[f64],
        scale: &[f64],
    ) -> bool {
        use crate::math::linear::AsIndex;
        for r in netlist.all_references() {
            let Some(i) = r.as_index() else { continue };
            if i >= residual.len() {
                continue;
            }
            let abs_limit = if r.variable().is_branch() { self.abstol } else { self.vntol_for(r.variable()) };
            let tol = abs_limit + self.reltol * scale[i];
            if residual[i].abs() > tol {
                return false;
            }
        }
        true
    }
}

// ── Policy (mutable, owned by drivers/plan) ────────────────────────────────

/// Convergence tunables the Newton loop consults each solve. Owned by the
/// driver (each analysis solver carries its own), never by the shared
/// immutable `Context` (MD-04). Hosts configure it per analysis.
#[derive(Debug, Clone)]
pub struct Policy {
    pub max_iter: usize,
    pub dc_damp_tolerance: f64,
    /// Diagnostic trace toggles (SS-08). `Default` seeds them from the
    /// `PIPERINE_TRACE_{GMIN,SRC,TRAN}` env vars — the single env read left;
    /// every trace site reads these typed fields.
    pub trace: TraceFlags,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_iter: 500,
            dc_damp_tolerance: 0.5,
            trace: TraceFlags::from_env(),
        }
    }
}

// ── Context (shared, immutable) ────────────────────────────────────────────

/// The check posture (DVC-12): how an analysis reacts when a validation
/// finding fires or a `require` margin crosses zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckPosture {
    /// A `require` margin below zero — or any `Error` finding — fails the
    /// analysis loud, naming the constraint, the instance path, the time,
    /// and the margin value. The default: a silent pass is never the
    /// default posture.
    #[default]
    Strict,
    /// Margins and findings are recorded (worst margin with its argmin);
    /// the analysis always completes. The optimizer's inner-loop posture —
    /// an infeasible iterate is a margin to climb out of, not an exception.
    Collect,
    /// Constraint kernels are never called and the findings hook is never
    /// polled — zero cost, nothing reported.
    Off,
}

/// `cover`'s own posture (dv-tester T33, DVT-41, D9) — separate from
/// [`CheckPosture`] so an optimizer's inner loop (`checks=Collect`, treating
/// an infeasible iterate as a margin to climb out of) does not also pollute
/// the coverage database with iterates nobody meant as verification runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoverPosture {
    /// `cover` bins are evaluated and their counters incremented on every
    /// accepted point. The default: coverage collection is normal
    /// verification behavior, not opt-in.
    #[default]
    On,
    /// No coverage kernel is called at all — zero cost, the database is
    /// left completely untouched.
    Off,
}

/// Which analyses an unscoped `require` holds in (D2). The scope is a
/// [`Context`] field, not a hard-coded rule — the default is every
/// analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequireScope {
    /// An unscoped `require` holds in every analysis (D2's default).
    #[default]
    EveryAnalysis,
}

/// Which analysis is currently running, used to gate scoped requires
/// (dv-core T28, DVC-19). A `require` inside `@ dc { … }` is evaluated
/// only when the running analysis is `Dc`; an analysis that never runs
/// leaves the scoped require "not exercised" (never a silent vacuous
/// pass). The names match the constraint event-term names (`dc`/`tran`/`ac`)
/// exactly — the `as_str` is the comparison key the device reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisKind {
    Dc,
    Tran,
    Ac,
}

impl AnalysisKind {
    /// The analysis's event-term name — the comparison key the device
    /// matches against a margin's `analyses` set.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dc => "dc",
            Self::Tran => "tran",
            Self::Ac => "ac",
        }
    }
}

/// The shared context every analysis receives: the immutable
/// [`Tolerances`], the check posture, and the unscoped-`require` scope.
/// Mutable convergence state lives on [`Policy`], owned by the driver;
/// simulation time reaches elements through their analysis context or as an
/// explicit argument (MD-04). The posture fields are pure run data —
/// changing them between analyses triggers no recompilation (MD-18).
///
/// `Debug`/`Default` are hand-written — see the impls below.
#[derive(Clone)]
pub struct Context {
    pub tolerances: Tolerances,
    /// How the analysis reacts to findings and margin crossings (DVC-12).
    pub checks: CheckPosture,
    /// Which analyses an unscoped `require` holds in (D2).
    pub require_scope: RequireScope,
    /// `cover`'s own posture (dv-tester T33, DVT-41, D9) — separate from
    /// [`Self::checks`].
    pub cover: CoverPosture,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("tolerances", &self.tolerances)
            .field("checks", &self.checks)
            .field("require_scope", &self.require_scope)
            .field("cover", &self.cover)
            .finish()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self {
            tolerances: Tolerances::default(),
            checks: CheckPosture::default(),
            require_scope: RequireScope::default(),
            cover: CoverPosture::default(),
        }
    }
}

impl Context {
    pub fn init_global() {
        INIT.call_once(|| {
            // Keep piperine's own diagnostics at INFO, but silence the Cranelift
            // JIT backend (it logs every compiled function's CL IR at INFO via
            // the `log` crate, which drowns real output). `cranelift*=off` keeps
            // the log-compat shim on for any other crate without the IR spam.
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::new("info,cranelift_jit=off,cranelift_codegen=off"),
                )
                .with_thread_ids(true)
                .with_thread_names(true)
                .init();

            set_global_parallelism(Par::Rayon(NonZeroUsize::new(1).unwrap()));
        });
    }
}
