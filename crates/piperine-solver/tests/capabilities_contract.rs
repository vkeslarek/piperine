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
        "BYPASS_OK" => {
            "consumed: the DC stamp-bypass cache is gated on every element declaring it \
             (analyses/dc.rs `bypass_allowed`); codegen declares it only for devices whose \
             DC stamps are a pure function of terminal voltages (P6/CLN-12)"
        }
        "SUPPORTS_ROLLBACK" => {
            "consumed: transient reject path + DC homotopy retry call \
             Element::checkpoint_state before each attempt and restore_state \
             on rejection (analyses/transient.rs, analyses/convergence.rs)"
        }
        // ── Jacobian / derivative capability (ABI-23) ───────────────────────
        "EMITS_VALIDATION" => {
            "consumed: validation findings poll — CircuitInstance::validation_reports \
             (core/circuit.rs) asks only elements declaring it, called at the DC final \
             converged solution (analyses/dc.rs) and on each accepted transient step \
             (analyses/transient.rs) (DVC-14)"
        }
        "HAS_SENSITIVITY" => {
            "consumed: the DC adjoint driver asks only elements declaring this for \
             analytic `AnalogDevice::param_stamps`; an element without it falls back to \
             stamp perturbation at the same operating point (analyses/sensitivity.rs, \
             DVG-05/06/07)"
        }
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
    // The flags dropped by SS-10 had a producer but no consumer, and
    // `SUPPORTS_QUERIES` (P6/CLN-11) had neither. None may reappear on the ABI
    // surface.
    for gone in ["LINEAR", "ANALYTIC_JACOBIAN", "STAMPS_CHARGE", "SUPPORTS_QUERIES"] {
        let present = ElementCapabilities::all()
            .iter_names()
            .any(|(name, _)| name == gone);
        assert!(!present, "removed write-only flag `{gone}` reappeared on ElementCapabilities");
    }
}

/// P6/CLN-13: no bit may sit on the ABI as "reserved" forever. Every registry
/// entry must name a live consumer — a branch gate, a loader, a driver — so the
/// state P6 ended (two bits nothing read) cannot quietly return.
///
/// The check is on the registry text because that text *is* the claim: an entry
/// that cannot name where the solver reads the flag is describing an intention,
/// not a contract.
#[test]
fn no_capability_flag_is_merely_reserved() {
    let merely_reserved: Vec<String> = ElementCapabilities::all()
        .iter_names()
        .filter_map(|(name, _)| {
            let entry = documented_consumer(name)?;
            let lowered = entry.to_ascii_lowercase();
            (lowered.starts_with("reserved") || lowered.contains("no consumer"))
                .then(|| format!("{name}: {entry}"))
        })
        .collect();

    assert!(
        merely_reserved.is_empty(),
        "capability flags claiming no live consumer (wire them or remove them — P6/CLN-13):\n  {}",
        merely_reserved.join("\n  ")
    );
}
