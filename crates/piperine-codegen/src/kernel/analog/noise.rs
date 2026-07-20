//! Noise capability: per-source PSD rows and flicker exponents.

use crate::resolve::NodeId;

use super::{AnalogCapability, AnalogFn};

/// Compiled noise sources. Present (`Some`) exactly when the analog body
/// declares at least one noise source.
pub(super) struct Noise {
    /// PSD per source, evaluated against `SimCtx.frequency`.
    pub(super) source: AnalogFn,
    /// Terminals `(plus, minus)` per noise source.
    pub(super) terminals: Vec<(NodeId, NodeId)>,
    /// Per-source flicker exponents (`0` for white noise): `S(f) = psd *
    /// (1 / f)^exponent`. `None` when every source is white.
    pub(super) exponents: Option<AnalogFn>,
}

impl AnalogCapability for Noise {
    fn count(&self) -> usize {
        self.terminals.len()
    }
}
