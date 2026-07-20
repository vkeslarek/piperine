//! Reactive (`ddt`) capability: the compiled charge `Q(V)` and its Jacobian.

use super::{AnalogCapability, AnalogFn};

/// Charge `Q(V)` and its Jacobian for a module's reactive (`ddt`)
/// contributions. Present (`Some`) exactly when the analog body has at
/// least one reactive contribution — `AnalogKernel::has_reactive` is
/// `self.reactive.is_some()`.
pub(super) struct Reactive {
    pub(super) charge: AnalogFn,
    pub(super) charge_jacobian: AnalogFn,
}

impl AnalogCapability for Reactive {
    /// A module has exactly one charge/Jacobian pair.
    fn count(&self) -> usize {
        1
    }
}
