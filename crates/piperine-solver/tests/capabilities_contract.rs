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
        "SUPPORTS_ROLLBACK" => {
            "consumed: transient reject path + DC homotopy retry call \
             Element::checkpoint_state before each attempt and restore_state \
             on rejection (analyses/transient.rs, analyses/convergence.rs)"
        }
        "SUPPORTS_QUERIES" => "reserved: host query-metadata hint; no solver consumer today (SS-11 audit)",
        // ── Jacobian / derivative capability (ABI-23) ───────────────────────
        "HAS_DISTO2" => {
            "consumed: .disto driver pre-scan — a device declaring this \
             contributes second-order nonlinear currents (HD2); the driver \
             warns when no device sets it (analyses/disto.rs, ABI-24)"
        }
        "HAS_DISTO3" => {
            "consumed: .disto driver pre-scan — a device declaring this \
             contributes third-order nonlinear currents (HD3); the driver \
             warns when no device sets it (analyses/disto.rs, ABI-24)"
        }
        "NUMERIC_JACOBIAN" => {
            "consumed: .disto driver pre-scan fail-loud — a device declaring \
             this has a finite-difference Jacobian and cannot provide the \
             analytic Hessian .disto requires; the driver errors (analyses/disto.rs, ABI-25)"
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
    // The flags dropped by SS-10 had a producer but no consumer. They must not
    // reappear on the ABI surface.
    for gone in ["LINEAR", "ANALYTIC_JACOBIAN", "STAMPS_CHARGE"] {
        let present = ElementCapabilities::all()
            .iter_names()
            .any(|(name, _)| name == gone);
        assert!(!present, "removed write-only flag `{gone}` reappeared on ElementCapabilities");
    }
}

/// ABI-23: the Jacobian/derivative capability bits occupy the documented bit
/// positions (`1 << 12`, `1 << 13`, `1 << 14`), compose with the existing
/// participation flags without collision, and are independently testable
/// through `contains`.
#[test]
fn jacobian_capability_bits_compose_correctly() {
    use piperine_solver::abi::ElementCapabilities as EC;

    // The three new bits are distinct and at the documented positions.
    assert_eq!(EC::HAS_DISTO2.bits(), 1u32 << 12);
    assert_eq!(EC::HAS_DISTO3.bits(), 1u32 << 13);
    assert_eq!(EC::NUMERIC_JACOBIAN.bits(), 1u32 << 14);

    // They compose with each other and with the prior flags.
    let analytic_nonlinear = EC::ANALOG | EC::LOADS_DC | EC::HAS_DISTO2 | EC::HAS_DISTO3;
    assert!(analytic_nonlinear.contains(EC::HAS_DISTO2));
    assert!(analytic_nonlinear.contains(EC::HAS_DISTO3));
    assert!(!analytic_nonlinear.contains(EC::NUMERIC_JACOBIAN));

    // A numeric-only device declares the numeric bit but not the disto bits.
    let numeric = EC::ANALOG | EC::NUMERIC_JACOBIAN;
    assert!(numeric.contains(EC::NUMERIC_JACOBIAN));
    assert!(!numeric.contains(EC::HAS_DISTO2));
    assert!(!numeric.contains(EC::HAS_DISTO3));

    // A purely linear device (resistor) declares none of the derivative bits.
    let linear = EC::ANALOG | EC::LOADS_DC;
    assert!(!linear.contains(EC::HAS_DISTO2));
    assert!(!linear.contains(EC::HAS_DISTO3));
    assert!(!linear.contains(EC::NUMERIC_JACOBIAN));

    // No overlap with the highest prior bit (BYPASS_OK = 1 << 11).
    assert_eq!(EC::BYPASS_OK.bits() & (EC::HAS_DISTO2 | EC::HAS_DISTO3 | EC::NUMERIC_JACOBIAN).bits(), 0);
}
