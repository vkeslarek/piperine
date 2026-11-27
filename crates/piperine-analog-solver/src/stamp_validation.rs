//! E0507 (§17.1 rule 4) — a non-finite device stamp names the branch.
//!
//! A contribution or a stamp shall not be NaN or infinity. Checked right
//! where a single device's own stamps are collected (the assembly's
//! `assemble_dc`/`assemble_tran` return value), *before* they ever reach the
//! shared linear system. This is what makes the branch nameable: by the time
//! a non-finite value has been summed into the assembled matrix/RHS
//! alongside every other device's contributions, there is no way back to
//! which one produced it — the `math/newton_raphson.rs` guard (checking the
//! *solved* vector after a full Newton iteration) can only report the
//! singular system, not a named branch.

use crate::error::{Error, RunCode, SolverDomain};
use crate::math::linear::Stamp;
use crate::netlist::AnalogReference;

/// Returns `None` when every stamp is finite (the common case, checked
/// with zero allocation); otherwise an `E0507` of `SolverDomain::Device`
/// naming `device_name` and the row (and column) of the first stamp found.
pub fn non_finite_stamp_error(device_name: &str, stamps: &[Stamp<AnalogReference, f64>]) -> Option<Error> {
    let at = stamps.iter().find_map(|s| match s {
        Stamp::Matrix(r, c, v) if !v.is_finite() => {
            Some(format!("row {}, column {}: {v}", r.idx()?, c.idx()?))
        }
        Stamp::Rhs(r, v) if !v.is_finite() => Some(format!("row {} of the right-hand side: {v}", r.idx()?)),
        _ => None,
    })?;
    Some(Error::run(
        RunCode::E0507,
        SolverDomain::Device,
        format!(
            "device `{device_name}` contributed a NaN or infinite stamp at {at} (§17.1 rule 4: a \
             contribution or stamp shall not be NaN or infinity)"
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::{AnalogVariable, NodeIdentifier};
    use std::sync::Arc;

    /// Builds the unknown references the tests stamp.
    struct ReferenceCases;

    impl ReferenceCases {
        fn unknown(n: usize, index: usize) -> AnalogReference {
            AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(n))), index)
        }
    }

    #[test]
    fn a_non_finite_stamp_is_e0507_with_its_row() {
        let (a, b) = (ReferenceCases::unknown(1, 3), ReferenceCases::unknown(2, 5));
        let stamps = vec![
            Stamp::Matrix(a.clone(), a.clone(), 1.0),
            Stamp::Matrix(b.clone(), a.clone(), f64::NAN),
            Stamp::Rhs(b.clone(), 2.0),
        ];
        let err = non_finite_stamp_error("r1", &stamps).expect("a NaN stamp");
        assert_eq!(err.code(), Some(RunCode::E0507));
        let text = err.to_string();
        assert!(text.contains("device `r1`"), "{text}");
        assert!(text.contains("row 5, column 3"), "the NaN is at row 5, column 3: {text}");
        assert!(!text.contains("LRM"), "{text}");

        let rhs = vec![Stamp::Rhs(b, f64::INFINITY)];
        let text = non_finite_stamp_error("r1", &rhs).expect("an infinite right side").to_string();
        assert!(text.contains("row 5 of the right-hand side"), "{text}");

        assert!(non_finite_stamp_error("r1", &[Stamp::Matrix(a.clone(), a, 1.0)]).is_none());
    }
}
