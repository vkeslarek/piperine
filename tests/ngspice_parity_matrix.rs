//! The ngspice **model × domain parity coverage gate** (NGP-15, MD-31).
//!
//! A parity claim no test exercises is unfalsifiable, so every one of the 13
//! shipped models × 6 cross-checked domains must be accounted for here: either
//! `Covered` by a golden fixture twin under `tests/ngspice/`, or
//! `NotApplicable` with a written reason. The asserts make that mechanical —
//! a cell that is neither fails the gate; a `Covered` row whose fixture twin
//! is missing, or that no golden target actually runs, fails loud.
//!
//! The model axis is the 13 model-card device types of `headers/spice/`
//! (spec.md's "13 ngspice-faithful device models"): res, cap, ind, sw, csw,
//! dio, jfet, bjt, mos1, mos2, mos3, tline and urc. The ideal sources
//! (vsrc/isrc), the controlled sources (vcvs/vccs/ccvs/cccs) and the coupling
//! elements (mut/xfmr) carry no ngspice `.model` card, so they are not rows
//! of the axis; their per-domain facts (ngspice's controlled sources are
//! noiseless, for one) are design.md's `NotApplicable` example, recorded
//! there rather than as cells.
//!
//! A `NotApplicable` reason starting with "pending" is a known coverage hole
//! and names the task or fixture that closes it; any other reason is a
//! recorded fact about the model (a capacitor is an open circuit at DC).
//! Later tasks flip pending cells to `Covered` as their fixtures land.

use std::path::PathBuf;

/// The six cross-checked domains — design.md's `Domain` enum, one golden
/// oracle each: DC operating point, DC sweep, AC, noise, transient and
/// temperature.
#[derive(Clone, Copy, PartialEq)]
enum Domain {
    DcOp,
    DcSweep,
    Ac,
    Noise,
    Transient,
    Temperature,
}

impl Domain {
    /// Every domain, so the exhaustiveness check cannot pass by forgetting one.
    const ALL: [Self; 6] = [
        Self::DcOp,
        Self::DcSweep,
        Self::Ac,
        Self::Noise,
        Self::Transient,
        Self::Temperature,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::DcOp => "DC operating point",
            Self::DcSweep => "DC sweep",
            Self::Ac => "AC",
            Self::Noise => "noise",
            Self::Transient => "transient",
            Self::Temperature => "temperature",
        }
    }
}

/// What backs one (model, domain) cell: the golden fixture that covers it,
/// or the written reason it is not applicable. An unreasoned hole is a
/// failure; a reasoned one is a record (MD-31).
#[derive(Clone, Copy)]
enum Coverage {
    Covered { fixture: &'static str },
    NotApplicable { reason: &'static str },
}

/// One row of the matrix: a model and its six domain cells. The array is the
/// per-model half of exhaustiveness — a row with five or seven cells does
/// not compile.
struct ModelRow {
    model: &'static str,
    cells: [(Domain, Coverage); 6],
}

/// The coverage registry and every check the gate runs. The `#[test]` fns
/// below are thin calls into it, so the failure text is built in one place.
struct ParityMatrix;

impl ParityMatrix {
    /// The model axis — the 13 model-card device types (see the file header).
    const MODELS: [&'static str; 13] = [
        "res", "cap", "ind", "sw", "csw", "dio", "jfet", "bjt", "mos1", "mos2", "mos3",
        "tline", "urc",
    ];

    /// The golden targets whose sources claim fixtures — a `Covered` cell is
    /// only falsifiable if one of these files actually runs the fixture.
    const GOLDEN_TARGETS: [&'static str; 5] = [
        "ngspice_validation.rs",
        "ngspice_ac.rs",
        "ngspice_noise.rs",
        "ngspice_tran.rs",
        "ngspice_temp.rs",
    ];

    /// The registry: every (model, domain) cell, covered or reasoned. Cells
    /// are `Covered` only where a real golden fixture exists today; the rest
    /// carry their reason, so the gate is honest about what is not yet
    /// measured.
    const REGISTRY: [ModelRow; 13] = [
        ModelRow {
            model: "res",
            cells: [
                (Domain::DcOp, Coverage::Covered { fixture: "divider" }),
                (Domain::DcSweep, Coverage::Covered { fixture: "diode_iv" }),
                (Domain::Ac, Coverage::Covered { fixture: "ac_rc" }),
                (Domain::Noise, Coverage::Covered { fixture: "noise_rc" }),
                (Domain::Transient, Coverage::Covered { fixture: "tran_rc" }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: temp_diode's series resistor runs at ngspice's zero \
                             tempco, so a res temperature defect would be invisible in it — \
                             needs a tempco-bearing fixture",
                }),
            ],
        },
        ModelRow {
            model: "cap",
            cells: [
                (Domain::DcOp, Coverage::NotApplicable {
                    reason: "a capacitor is an open circuit at DC — its only DC behaviour is \
                             its absence, which every AC/transient fixture already exercises",
                }),
                (Domain::DcSweep, Coverage::NotApplicable {
                    reason: "a capacitor conducts no DC — a swept-DC comparison would compare \
                             its absence",
                }),
                (Domain::Ac, Coverage::Covered { fixture: "ac_rc" }),
                (Domain::Noise, Coverage::Covered { fixture: "noise_rc" }),
                (Domain::Transient, Coverage::Covered { fixture: "tran_rc" }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps a temperature-dependent \
                             capacitance",
                }),
            ],
        },
        ModelRow {
            model: "ind",
            cells: [
                (Domain::DcOp, Coverage::NotApplicable {
                    reason: "pending: no golden fixture exercises ind's DC (short-circuit) \
                             behaviour",
                }),
                (Domain::DcSweep, Coverage::NotApplicable {
                    reason: "pending: no swept-DC golden fixture exercises ind",
                }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises ind",
                }),
                (Domain::Noise, Coverage::NotApplicable {
                    reason: "pending: no noise golden fixture exercises ind — reactive \
                             elements are noiseless in ngspice, so the assertion is its \
                             absence from the spectrum",
                }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises ind",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps ind across temperature",
                }),
            ],
        },
        ModelRow {
            model: "sw",
            cells: [
                (Domain::DcOp, Coverage::NotApplicable {
                    reason: "pending: no golden fixture exercises the switch's on/off \
                             operating point",
                }),
                (Domain::DcSweep, Coverage::NotApplicable {
                    reason: "pending: no swept-DC golden fixture exercises the switch",
                }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises the switch",
                }),
                (Domain::Noise, Coverage::NotApplicable {
                    reason: "pending: T13 adds the ON-state thermal-noise fixture (ngspice \
                             swnoise.c)",
                }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises the switch's \
                             hysteresis",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps the switch across temperature",
                }),
            ],
        },
        ModelRow {
            model: "csw",
            cells: [
                (Domain::DcOp, Coverage::NotApplicable {
                    reason: "pending: no golden fixture exercises the current-controlled \
                             switch's operating point",
                }),
                (Domain::DcSweep, Coverage::NotApplicable {
                    reason: "pending: no swept-DC golden fixture exercises the \
                             current-controlled switch",
                }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises the current-controlled \
                             switch",
                }),
                (Domain::Noise, Coverage::NotApplicable {
                    reason: "pending: T13 adds the ON-state thermal-noise fixture (ngspice \
                             swnoise.c)",
                }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises the \
                             current-controlled switch",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps the current-controlled switch \
                             across temperature",
                }),
            ],
        },
        ModelRow {
            model: "dio",
            cells: [
                (Domain::DcOp, Coverage::Covered { fixture: "rdiode" }),
                (Domain::DcSweep, Coverage::Covered { fixture: "diode_iv" }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises the diode's junction \
                             capacitance",
                }),
                (Domain::Noise, Coverage::NotApplicable {
                    reason: "pending: no noise golden fixture exercises the diode's \
                             shot/flicker sources",
                }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises the diode's \
                             charge storage",
                }),
                (Domain::Temperature, Coverage::Covered { fixture: "temp_diode" }),
            ],
        },
        ModelRow {
            model: "jfet",
            cells: [
                (Domain::DcOp, Coverage::Covered { fixture: "jfet_bias" }),
                (Domain::DcSweep, Coverage::Covered { fixture: "jfet_id_vds" }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises the jfet",
                }),
                (Domain::Noise, Coverage::NotApplicable {
                    reason: "pending: T12 replaces the saturation-only gm_est with the \
                             analytic gm, adds nlev=3, and lands the fixture",
                }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises the jfet",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps the jfet across temperature",
                }),
            ],
        },
        ModelRow {
            model: "bjt",
            cells: [
                (Domain::DcOp, Coverage::Covered { fixture: "bjt_ce" }),
                (Domain::DcSweep, Coverage::NotApplicable {
                    reason: "pending: no swept-DC golden fixture exercises the bjt",
                }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises the bjt",
                }),
                (Domain::Noise, Coverage::NotApplicable {
                    reason: "pending: no noise golden fixture exercises the bjt's six noise \
                             sources",
                }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises the bjt",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: T15 temperature-scales VAF/VAR and lands the fixture",
                }),
            ],
        },
        ModelRow {
            model: "mos1",
            cells: [
                (Domain::DcOp, Coverage::Covered { fixture: "nmos_fixed" }),
                (Domain::DcSweep, Coverage::Covered { fixture: "nmos_id_vgs" }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises mos1",
                }),
                (Domain::Noise, Coverage::Covered { fixture: "noise_mos_linear" }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises mos1",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps mos1 across temperature",
                }),
            ],
        },
        ModelRow {
            model: "mos2",
            cells: [
                (Domain::DcOp, Coverage::Covered { fixture: "nmos2_fixed" }),
                (Domain::DcSweep, Coverage::Covered { fixture: "nmos2_id_vds" }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises mos2",
                }),
                (Domain::Noise, Coverage::Covered { fixture: "noise_mos2_linear" }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises mos2",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps mos2 across temperature",
                }),
            ],
        },
        ModelRow {
            model: "mos3",
            cells: [
                (Domain::DcOp, Coverage::Covered { fixture: "nmos3_fixed" }),
                (Domain::DcSweep, Coverage::Covered { fixture: "nmos3_id_vds" }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises mos3",
                }),
                (Domain::Noise, Coverage::Covered { fixture: "noise_mos3_linear" }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises mos3",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps mos3 across temperature",
                }),
            ],
        },
        ModelRow {
            model: "tline",
            cells: [
                (Domain::DcOp, Coverage::NotApplicable {
                    reason: "pending: no ngspice golden twin exercises tline (tests/tline.rs \
                             covers it in-process, not against ngspice)",
                }),
                (Domain::DcSweep, Coverage::NotApplicable {
                    reason: "pending: no ngspice golden twin exercises tline",
                }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no ngspice golden twin exercises tline",
                }),
                (Domain::Noise, Coverage::NotApplicable {
                    reason: "the lossless line is noiseless in ngspice, and no noise golden \
                             twin asserts its absence from a spectrum",
                }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no ngspice golden twin exercises tline",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no ngspice golden twin exercises tline",
                }),
            ],
        },
        ModelRow {
            model: "urc",
            cells: [
                (Domain::DcOp, Coverage::Covered { fixture: "urc_lump2" }),
                (Domain::DcSweep, Coverage::NotApplicable {
                    reason: "pending: the lump fixtures are operating-point checks; no \
                             swept-DC golden fixture exercises the urc ladder",
                }),
                (Domain::Ac, Coverage::NotApplicable {
                    reason: "pending: no AC golden fixture exercises the urc ladder",
                }),
                (Domain::Noise, Coverage::NotApplicable {
                    reason: "pending: no noise golden fixture exercises the urc ladder",
                }),
                (Domain::Transient, Coverage::NotApplicable {
                    reason: "pending: no transient golden fixture exercises the urc ladder",
                }),
                (Domain::Temperature, Coverage::NotApplicable {
                    reason: "pending: no golden fixture sweeps the urc ladder across \
                             temperature",
                }),
            ],
        },
    ];

    fn circuits_dir() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ngspice"))
    }

    /// Cells that are neither covered nor reasoned: a model with no row, a
    /// duplicated row, a row naming a model outside the axis, or a row whose
    /// six cells do not hold each domain exactly once.
    fn unaccounted_cells() -> Vec<String> {
        let mut problems = Vec::new();
        for model in Self::MODELS {
            match Self::REGISTRY.iter().filter(|row| row.model == model).count() {
                1 => {}
                0 => problems.push(format!("{model}: no registry row")),
                n => problems.push(format!("{model}: {n} registry rows — a duplicate")),
            }
        }
        for row in &Self::REGISTRY {
            if !Self::MODELS.contains(&row.model) {
                problems.push(format!("{}: not one of the 13 models", row.model));
            }
            for domain in Domain::ALL {
                match row.cells.iter().filter(|(d, _)| *d == domain).count() {
                    1 => {}
                    0 => problems
                        .push(format!("{}: no cell for the {} domain", row.model, domain.name())),
                    n => problems.push(format!(
                        "{}: the {} domain appears {n} times",
                        row.model,
                        domain.name()
                    )),
                }
            }
        }
        problems
    }

    /// A `Covered` row names a fixture whose `.cir`/`.phdl` twin must exist
    /// and be non-empty — a row naming a missing twin is a claim no test
    /// backs, which is exactly what this gate exists to catch.
    fn missing_fixtures() -> Vec<String> {
        let mut missing = Vec::new();
        for row in &Self::REGISTRY {
            for (domain, coverage) in &row.cells {
                let Coverage::Covered { fixture } = coverage else { continue };
                for extension in ["cir", "phdl"] {
                    let path = Self::circuits_dir().join(format!("{fixture}.{extension}"));
                    let Ok(meta) = std::fs::metadata(&path) else {
                        missing.push(format!(
                            "{} ({}): fixture twin {} does not exist",
                            row.model,
                            domain.name(),
                            path.display()
                        ));
                        continue;
                    };
                    if meta.len() == 0 {
                        missing.push(format!(
                            "{} ({}): fixture twin {} is empty",
                            row.model,
                            domain.name(),
                            path.display()
                        ));
                    }
                }
            }
        }
        missing
    }

    /// A `Covered` fixture that no golden target references is an
    /// unfalsifiable claim — the row says "covered" and no test runs it.
    fn unclaimed_fixtures() -> Vec<String> {
        let targets_dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests"));
        let sources: Vec<String> = Self::GOLDEN_TARGETS
            .iter()
            .map(|target| {
                let path = targets_dir.join(target);
                std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            })
            .collect();
        let mut unclaimed = Vec::new();
        for row in &Self::REGISTRY {
            for (domain, coverage) in &row.cells {
                let Coverage::Covered { fixture } = coverage else { continue };
                let quoted = format!("\"{fixture}\"");
                if !sources.iter().any(|text| text.contains(&quoted)) {
                    unclaimed.push(format!(
                        "{} ({}): fixture {fixture} appears in no golden target ({})",
                        row.model,
                        domain.name(),
                        Self::GOLDEN_TARGETS.join(", ")
                    ));
                }
            }
        }
        unclaimed
    }

    /// A `NotApplicable` cell with a blank reason is an unrecorded hole — the
    /// registry only means something if the reason is written down.
    fn unreasoned_cells() -> Vec<String> {
        let mut cells = Vec::new();
        for row in &Self::REGISTRY {
            for (domain, coverage) in &row.cells {
                let Coverage::NotApplicable { reason } = coverage else { continue };
                if reason.trim().is_empty() {
                    cells.push(format!("{} ({})", row.model, domain.name()));
                }
            }
        }
        cells
    }
}

/// The exhaustiveness assert: every one of the 13 models × 6 domains is
/// `Covered` or `NotApplicable` with a reason. An unaccounted cell — a new
/// model with no row, a dropped domain — fails here naming it.
#[test]
fn every_model_domain_cell_is_accounted_for() {
    let unaccounted = ParityMatrix::unaccounted_cells();
    assert!(
        unaccounted.is_empty(),
        "parity-matrix cells neither covered nor reasoned (every one of the 13 models × 6 \
         domains needs a cell — add the coverage or the written reason):\n  {}",
        unaccounted.join("\n  ")
    );
}

/// A `Covered` row whose `.cir`/`.phdl` twin is missing or empty fails loud
/// naming the cell and the path — the claim exists only if the fixture does.
#[test]
fn every_covered_cell_names_an_existing_fixture() {
    let missing = ParityMatrix::missing_fixtures();
    assert!(
        missing.is_empty(),
        "Covered parity-matrix cells whose fixture twin is missing or empty:\n  {}",
        missing.join("\n  ")
    );
}

/// A `Covered` fixture no golden target runs is the spec's headline failure —
/// an unfalsifiable claim — so the row must name a fixture one of the golden
/// targets actually invokes.
#[test]
fn every_covered_fixture_is_run_by_a_golden_target() {
    let unclaimed = ParityMatrix::unclaimed_fixtures();
    assert!(
        unclaimed.is_empty(),
        "Covered parity-matrix cells whose fixture no golden target runs (the claim is \
         unfalsifiable — wire the fixture into a target, or downgrade the cell):\n  {}",
        unclaimed.join("\n  ")
    );
}

/// `NotApplicable` is load-bearing only with its reason written down; a blank
/// reason is an unrecorded coverage hole.
#[test]
fn every_not_applicable_cell_states_a_reason() {
    let unreasoned = ParityMatrix::unreasoned_cells();
    assert!(
        unreasoned.is_empty(),
        "NotApplicable parity-matrix cells with no written reason:\n  {}",
        unreasoned.join("\n  ")
    );
}
