//! Contract test for `ElementCapabilities` (SS-10, SS-E): every surviving flag
//! must have a documented solver consumer — a place the solver either
//! branch-gates on it or consumes it as a per-analysis / loader descriptor.
//!
//! This is the reintroduction guard: the registry below must stay exhaustive
//! over `ElementCapabilities::all()`. A newly added flag with no registry
//! entry (i.e. a write-only bit like the removed `LINEAR` /
//! `ANALYTIC_JACOBIAN` / `STAMPS_CHARGE`) fails this test.
//!
//! Only `DIGITAL` and `HAS_INTERNAL_UNKNOWNS` are *branch-gated* today; the
//! rest are per-analysis participation descriptors (the analog/noise loaders
//! and the result mapper consume them) or a reserved bit owned by a named
//! follow-up. Each entry names how the solver relates to the flag.

use piperine_solver::abi::ElementCapabilities;

/// The documented solver consumer (or reserved owner) for each capability
/// flag. Returns `None` for any flag not accounted for — that is the failure
/// signal for a reintroduced write-only bit.
fn documented_consumer(flag_name: &str) -> Option<&'static str> {
    Some(match flag_name {
        // ── Branch-gated: the solver reads these to decide control flow ──────
        "DIGITAL" => {
            "branch-gated: DcSolver::solve mixed-signal loop (solver/dc.rs), \
             DigitalTopology scheduler (digital/scheduler.rs), \
             CircuitInstance::init_digital (core/circuit.rs)"
        }
        "HAS_INTERNAL_UNKNOWNS" => {
            "branch-gated: CircuitBuilder unknown-allocation seam (core/builder.rs)"
        }
        // ── Engine participation descriptors (loaders iterate + consume) ─────
        "ANALOG" => "descriptor: analog engine participation (MNA loaders)",
        "SAMPLES_ANALOG" => {
            "descriptor: A2D — device is fed the analog slice each digital \
             evaluation (digital/scheduler.rs, core/circuit.rs)"
        }
        "LOADS_DC" => "descriptor: contributes to the DC operating point (solver/dc.rs)",
        "LOADS_AC" => "descriptor: contributes to the AC sweep (solver/ac.rs)",
        "LOADS_TRAN" => "descriptor: contributes to transient integration (solver/transient.rs)",
        "EMITS_NOISE" => "descriptor: returns noise sources (solver/noise.rs)",
        "DEPENDS_ON_DIGITAL" => {
            "descriptor: analog load reads the digital snapshot (D2A ordering)"
        }
        // ── Reserved bits owned by a named follow-up feature ─────────────────
        "BYPASS_OK" => "reserved: solver-performance owns stamp bypass",
        "SUPPORTS_ROLLBACK" => "reserved: solver-commit-rollback owns the lifecycle",
        "SUPPORTS_QUERIES" => "descriptor: host may skip the default opvar scan (core/introspect.rs)",
        _ => return None,
    })
}

#[test]
fn every_surviving_capability_flag_is_documented() {
    let undocumented: Vec<String> = ElementCapabilities::all()
        .iter_names()
        .filter(|(name, _)| documented_consumer(name).is_none())
        .map(|(name, _)| name.to_string())
        .collect();

    assert!(
        undocumented.is_empty(),
        "ElementCapabilities flags without a documented solver consumer \
         (write-only bits must be removed, not reintroduced): {undocumented:?}"
    );
}

#[test]
fn removed_write_only_flags_stay_gone() {
    // The flags dropped by SS-10 had a producer but no consumer. They must not
    // reappear on the ABI surface.
    for gone in ["LINEAR", "ANALYTIC_JACOBIAN", "STAMPS_CHARGE"] {
        let present = ElementCapabilities::all()
            .iter_names()
            .any(|(name, _)| name == gone);
        assert!(!present, "removed write-only flag `{gone}` reappeared on ElementCapabilities");
    }
}
