//! The check posture and `require` scope on `Context` (DVC-12, D2):
//! `checks` defaults to `Strict`, `require_scope` defaults to "every
//! analysis"; both fields are readable and settable; the enums export on
//! both MD-17 tiers.

use piperine_solver::abi::{CheckPosture as AbiPosture, RequireScope as AbiScope};
use piperine_solver::prelude::{CheckPosture, Context, RequireScope};

/// DVC-12 / D2: the defaults are `Strict` and "every analysis" — a silent
/// pass is never the default posture, and an unscoped `require` holds
/// everywhere unless the field says otherwise.
#[test]
fn context_defaults_to_strict_and_every_analysis() {
    let ctx = Context::default();
    assert_eq!(ctx.checks, CheckPosture::Strict, "the default posture is strict (DVC-12)");
    assert_eq!(
        ctx.require_scope,
        RequireScope::EveryAnalysis,
        "an unscoped require holds in every analysis by default (D2)"
    );
    assert_eq!(CheckPosture::default(), CheckPosture::Strict);
    assert_eq!(RequireScope::default(), RequireScope::EveryAnalysis);
}

/// DVC-12: both fields are readable and settable — the three postures
/// round-trip through the field.
#[test]
fn posture_fields_are_readable_and_settable() {
    for posture in [CheckPosture::Collect, CheckPosture::Off, CheckPosture::Strict] {
        let ctx = Context { checks: posture, ..Context::default() };
        assert_eq!(ctx.checks, posture, "the field carries {posture:?}");
    }
    assert_ne!(CheckPosture::Strict, CheckPosture::Collect);
    assert_ne!(CheckPosture::Collect, CheckPosture::Off);
    assert_ne!(CheckPosture::Off, CheckPosture::Strict);
}

/// MD-17: the posture enums are reachable from both the host tier
/// (`prelude::`) and the element tier (`abi::`), naming the same types.
#[test]
fn posture_enums_export_on_both_tiers() {
    let from_prelude = CheckPosture::Collect;
    let from_abi: AbiPosture = from_prelude;
    assert_eq!(from_prelude, from_abi);
    let scope_prelude = RequireScope::EveryAnalysis;
    let scope_abi: AbiScope = scope_prelude;
    assert_eq!(scope_prelude, scope_abi);
}
