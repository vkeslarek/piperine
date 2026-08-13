//! `cover` bin-mapper capability (dv-tester T33, DVT-40): one compiled
//! function per declared `cover` point, evaluated exactly like a
//! declared pointwise margin (same [`super::AnalogFn`] shape, same
//! `compile_rows` compilation path in `compile.rs`) — the device side maps
//! the raw evaluated value into a bin index and increments a counter
//! (`device/analog/coverage.rs`), never tracking a worst/argmin the way a
//! margin does (D9: coverage is not a margin). `cover cross` entries carry
//! no compiled function at all — they are a host-side combination of
//! already-mapped bin indices, static catalog data only.

use crate::resolve::{ResolvedCoverBins, ResolvedWindow};

use super::{AnalogCapability, AnalogFn};

/// One declared `cover` point's provenance: its authored label, its
/// literal bin partition, the analyses its helpers belong to, and the
/// temporal window an enclosing event block scopes it to — the same shape
/// [`super::constraints::MarginInfo`] carries, minus the margin kind.
#[derive(Debug, Clone)]
pub struct CoverInfo {
    pub label: String,
    pub bins: ResolvedCoverBins,
    pub analyses: Vec<String>,
    pub window: Option<ResolvedWindow>,
}

/// One declared `cover cross`'s catalog entry — no compiled function; the
/// device combines the named points' already-mapped bin indices.
#[derive(Debug, Clone)]
pub struct CoverCrossInfo {
    pub label: String,
    pub points: Vec<String>,
    pub joint_bin_count: usize,
}

/// One compiled `cover` point: its provenance plus the JIT function
/// evaluating its pointwise value at a point (the bin-mapper reads this
/// value and maps it to a bin index device-side).
pub(super) struct CompiledCover {
    pub(super) info: CoverInfo,
    pub(super) eval: AnalogFn,
}

/// The compiled `cover` capability of one module, in authored order.
pub(super) struct Coverage {
    pub(super) points: Vec<CompiledCover>,
    /// The provenance catalog, parallel to [`points`](Self::points) — kept
    /// separate so it is shareable without the function pointers (same
    /// pattern as `Constraints::catalog`).
    pub(super) catalog: Vec<CoverInfo>,
    pub(super) crosses: Vec<CoverCrossInfo>,
}

impl Coverage {
    pub(super) fn new(points: Vec<CompiledCover>, crosses: Vec<CoverCrossInfo>) -> Self {
        let catalog = points.iter().map(|p| p.info.clone()).collect();
        Self { points, catalog, crosses }
    }
}

impl AnalogCapability for Coverage {
    fn count(&self) -> usize {
        self.points.len()
    }
}
