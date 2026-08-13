//! Constraint margin capability (DVC-10): one compiled function per
//! **declared** pointwise margin — never one per instance (the kernel is
//! `Arc`-shared across every instance of the module; the `.disto`
//! function-count explosion on per-instance emission is the standing proof
//! of what happens otherwise, design §E). Margin expressions reference the
//! residual's subexpressions through the same `__temp` tape the residual
//! reads, so shared work is CSE'd per evaluation rather than re-inlined.
//! Present (`Some`) exactly when the module declares at least one
//! pointwise margin; reduced-class margins never reach the kernel (D7 —
//! host-side reductions).

use crate::resolve::{MarginKind, ResolvedWindow};

use super::{AnalogCapability, AnalogFn};

/// The provenance one declared margin carries: its authored label (a
/// `require`'s name, a `target`'s `target#<index>` position), its
/// statement kind, the analyses its helpers belong to (DVC-09 — the
/// record a later cross-analysis read refuses on), and the temporal
/// window an enclosing event block scopes it to (DVC-20 — `None` is
/// every evaluation point of an in-scope analysis).
#[derive(Debug, Clone)]
pub struct MarginInfo {
    pub label: String,
    pub kind: MarginKind,
    pub analyses: Vec<String>,
    pub window: Option<ResolvedWindow>,
}

/// One compiled margin: its provenance plus the JIT function evaluating
/// its signed value (`m ≥ 0` ⟺ satisfied) at a point.
pub(super) struct CompiledMargin {
    pub(super) info: MarginInfo,
    pub(super) eval: AnalogFn,
}

/// The compiled constraint margins of one module, in authored order.
pub(super) struct Constraints {
    pub(super) margins: Vec<CompiledMargin>,
    /// The provenance catalog, parallel to [`margins`](Self::margins) —
    /// the device's reporting view (kept separate so the catalog is
    /// shareable without the function pointers).
    pub(super) catalog: Vec<MarginInfo>,
}

impl Constraints {
    /// One row per declared pointwise margin.
    pub(super) fn new(margins: Vec<CompiledMargin>) -> Self {
        let catalog = margins.iter().map(|m| m.info.clone()).collect();
        Self { margins, catalog }
    }
}

impl AnalogCapability for Constraints {
    fn count(&self) -> usize {
        self.margins.len()
    }
}
