//! Bundles run from MSL source on the Rust backend (feature `resolved-nets-bundles`, BN-02 to BN-06): each model is written
//! in `tests/bundles/`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is written from the rule it cites (§5, §7), never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_api::{Field, Streams};
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_solver::prelude::{Context, NodeIdentifier};
use piperine_parity::project::Project;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::{Built, Seen};
use support::{build, series};

/// One MSL file of `tests/bundles/`.
struct Model {
    /// The project read from that file.
    project: Project,
}

impl Model {
    /// The project of `tests/bundles/<file>`.
    fn of(file: &str) -> Model {
        let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bundles").join(file);
        Model { project: Project::new(&root) }
    }

    /// `top` elaborated and compiled once.
    fn built(&self, top: &str) -> Built {
        build(&self.project, top)
    }

    /// `v(<name>)` of the bench `top` at its operating point.
    fn operating_point(&self, top: &str, name: &str) -> f64 {
        let mut running = self.project.compile(top).unwrap_or_else(|r| panic!("{r}"));
        let node = NodeIdentifier::Anonymous(running.ir.node_at(&format!("{top}.{name}")).unwrap_or_else(|| panic!("no node `{top}.{name}`")).id().index());
        running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves").get_node(&node).expect("a node voltage")
    }

    /// The integer fields of every record of the stream `name` a run wrote, in order.
    fn records(built: &Built, seen: &Seen, name: &str) -> Vec<Vec<(String, i128)>> {
        let streams = Streams::of(built.ir.clone());
        seen.all()
            .iter()
            .filter_map(|o| streams.decode(o))
            .map(|r| r.unwrap_or_else(|f| panic!("{f}")))
            .filter(|r| r.stream == name)
            .map(|r| {
                r.fields
                    .into_iter()
                    .map(|(field, value)| match value {
                        Field::Int(value) => (field, value),
                        Field::Bool(value) => (field, i128::from(value)),
                        other => panic!("no integer field `{field}`: {other:?}"),
                    })
                    .collect()
            })
            .collect()
    }

    /// The `data` of each record of the stream `name`.
    fn data(built: &Built, seen: &Seen, name: &str) -> Vec<i128> {
        Model::records(built, seen, name).into_iter().map(|fields| fields.iter().find(|(field, _)| field == "data").map(|(_, value)| *value).expect("a `data` field")).collect()
    }
}

#[test]
fn a_producer_consumer_and_observer_run_three_beats() {
    // §7 rules 3, 7, 14: the producer drives `valid` and `data`, the consumer drives `ready`, the observer reads both and
    // drives none; one `Stream` signal joins the three. `ready` toggles at each rise (5, 15, 25 ns, ...), starting closed, so
    // a beat takes place at the rises 15, 35 and 55 ns, each carrying the next number: 1, 2, 3; after the third, `valid`
    // is false (`count < 3`) and no beat follows.
    let built = Model::of("stream.msl").built("HandshakeBench");
    let seen = built.run(120e-9, 1e-9);
    assert_eq!(Model::data(&built, &seen, "taken"), [1, 2, 3], "the consumer takes one number per beat");
    assert_eq!(Model::data(&built, &seen, "seen"), [1, 2, 3], "the observer sees the same beats");
    let valid: Vec<(i64, Vec<u8>)> = series(&seen, "HandshakeBench.s.valid");
    // `valid` starts true (`count < 3` at 0) and falls once, when the third beat's commit moves `count` to 3, at 55 ns
    assert_eq!(valid.first().map(|c| c.1.clone()), Some(vec![1]), "valid holds from (0, 0)");
    assert_eq!(valid.last().map(|c| (c.0, c.1.clone())), Some((55_000_000, vec![0])), "valid falls at the third beat: {valid:?}");
}

#[test]
fn beat_fires_on_rising_edges_with_valid_and_ready() {
    // §7 rule 13: `on rx.beat(clk)` runs exactly where `clk.rise() && rx.valid && rx.ready` does: `ready` toggles at each rise, so
    // the beats are the rises at 15, 35 and 55 ns, carrying 1, 2 and 3, and no beat follows the third (`valid` falls).
    let built = Model::of("stream.msl").built("BeatBench");
    let seen = built.run(120e-9, 1e-9);
    assert_eq!(Model::data(&built, &seen, "beats"), [1, 2, 3], "one record per beat, none at the other rising edges");
}

#[test]
fn field_commits_definitions_and_methods_follow_section_5() {
    // §7 rule 12, §5: `p.level` starts unknown, the `Default` of a `Logic` (§5.2.8), and `changed()` fires at (0, 0) for it;
    // the writer commits high at its second rise (15 ns, `rises == 1`) and low at its fifth (45 ns), and each change reads
    // the level it replaces with `prev()` (§5.3). `q.ack` is defined over the level: it rises once, with the level high.
    let built = Model::of("fields.msl").built("PipeBench");
    let seen = built.run(80e-9, 1e-9);
    let levels: Vec<(i128, i128, i128)> = Model::records(&built, &seen, "level").iter().map(|r| (r[0].1, r[1].1, r[2].1)).collect();
    assert_eq!(levels, [(0, 1, 0), (1, 0, 0), (0, 0, 1)], "(high, unknown, before high): unknown at (0, 0), then high after unknown, then low after high");
    assert_eq!(Model::records(&built, &seen, "acks"), [vec![("high".to_string(), 1)]], "`ack` rises once, when the level is high");
}

#[test]
fn a_diff_bundle_connects_its_nodes_like_ports() {
    // §7 rule 14, BN-03 AC 5: a source holds `v(d.p, d.n)` at 1 V, `d.n` is tied to ground through a field passed as a node,
    // and the load sits on the same pair: `v(pair.p)` = 1 V, the source's level.
    let diff = Model::of("diff.msl");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("diff bundle", "v(pair.p)", Coordinate::OperatingPoint), 1.0, diff.operating_point("DiffBench", "pair.p"));
    // `pair.n` is tied to ground, so the solver merges it with `gnd` (§6): every instance sees the pair as (`pair.p`, ground)
    let structure = diff.project.elaborate("DiffBench").unwrap_or_else(|r| panic!("{r}")).print_structural();
    for instance in ["DiffBench.w.s: Source", "DiffBench.w.l: Load"] {
        assert!(structure.contains(&format!("instance {instance}  nodes (DiffBench.pair.p, gnd)")), "{instance} sees the pair: {structure}");
    }
}

#[test]
fn vd_on_a_diff_bundle_gives_the_differential_voltage() {
    // §7 rule 13: `d.vd()` is `v(d.p, d.n)` through the handle `self`: the source holds the pair at 1 V, so the mirror, which
    // follows `2 * d.vd()`, sits at 2 V.
    let diff = Model::of("diff.msl");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("vd of a diff bundle", "v(m)", Coordinate::OperatingPoint), 2.0, diff.operating_point("VdBench", "m"));
}

#[test]
fn a_bundle_carrier_connects_field_by_field() {
    // §7 rule 14: a bundle port handed on as a bundle carrier (`Wrap` gives its `d` to a source and a load; `Relay` its `c` to a
    // holder) joins each field to the field of the same name: the source's and the load's pair is the bench's pair, and the
    // holder reached through `Relay` holds the bench's bit line at its 0.5 V and sees the word line rise once, at the first
    // rising edge, 5 ns.
    let cells = Model::of("cells.msl");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("relayed cell", "v(b)", Coordinate::OperatingPoint), 0.5, cells.operating_point("RelayBench", "b"));
    let built = cells.built("RelayBench");
    let seen = built.run(40e-9, 1e-9);
    let lines: Vec<i128> = Model::records(&built, &seen, "word_line").iter().map(|r| r[0].1).collect();
    assert_eq!(lines, [0, 1], "the line is low at (0, 0) and rises once");
}

#[test]
fn a_cell_literal_connects_each_named_field() {
    // §7 rule 8: `Cell { bl: b, wl: w }` names each field once, in an order of its own: `c.bl` is `b` (held at the holder's 0.5 V) and
    // `c.wl` is `w` (low at (0, 0), high from the first rising edge).
    let cells = Model::of("cells.msl");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("cell literal", "v(b)", Coordinate::OperatingPoint), 0.5, cells.operating_point("CellBench", "b"));
    let built = cells.built("CellBench");
    let seen = built.run(40e-9, 1e-9);
    let lines: Vec<i128> = Model::records(&built, &seen, "word_line").iter().map(|r| r[0].1).collect();
    assert_eq!(lines, [0, 1], "the word line changes at (0, 0) and at the first rising edge");
}

#[test]
fn nested_bundles_apply_flip_recursively() {
    // §7 rules 3 and 5: the producer `A` drives `fwd.x` (true) and `back.y`; the consumer `B` (the flipped view, applied to the
    // nested fields too) drives `fwd.y` and `back.x`. `fwd.y = fwd.x` = 1, `back.y = fwd.y` = 1 and `back.x = !back.y` = 0: if
    // `flip` were not applied to the nested `back`, two modules would drive one field (`E0220`) or none would.
    let built = Model::of("nested.msl").built("NestedBench");
    let seen = built.run(20e-9, 1e-9);
    let fields: Vec<Vec<(String, i128)>> = Model::records(&built, &seen, "fields");
    let expected = vec![vec![("fwd_x".to_string(), 1), ("fwd_y".to_string(), 1), ("back_x".to_string(), 0), ("back_y".to_string(), 1)]];
    assert_eq!(fields[..1], expected, "the four fields at the first rising edge");
}

#[test]
fn every_view_may_drive_a_resolved_field() {
    // §7 rule 4, §5.2.2, D8: `sda` is of a resolved nature, so it has no direction and the producer, the consumer and the observer
    // all drive it; it holds `strongest` of the drives (§22.3.2). Master: weak low, slave: pull high, spy: strong low. With the
    // spy the net is low at the strong strength; without it the pull wins, high at the pull strength. (The spy drives
    // nothing else: `scl`, a one-driver field, is the producer's alone.)
    let model = Model::of("resolved.msl");
    for (top, level, strength) in [("BusBench", 0.0, 3.0), ("QuietBench", 1.0, 2.0)] {
        let built = model.built(top);
        let seen = built.run(20e-9, 1e-9);
        let streams = Streams::of(built.ir.clone());
        let records: Vec<Vec<(String, f64)>> = seen
            .all()
            .iter()
            .filter_map(|o| streams.decode(o))
            .map(|r| r.unwrap_or_else(|f| panic!("{f}")))
            .filter(|r| r.stream == "sda")
            .map(|r| r.fields.into_iter().map(|(field, value)| match value {
                Field::Real(value) => (field, value),
                other => panic!("no real field `{field}`: {other:?}"),
            }).collect())
            .collect();
        assert_eq!(records.first(), Some(&vec![("level".to_string(), level), ("strength".to_string(), strength)]), "{top}: the settled `sda` at the first rise");
    }
}

#[test]
fn a_generic_bundle_is_instantiated_per_type_argument() {
    // §7 rule 11: `Pipe<T>` as `Pipe<u8>` and as `Pipe<u16>` are two bundles whose `data` fields hold an 8-bit and a 16-bit number:
    // the producers offer 200 and 3000, and each tap records the number of its own stream at the first rising edge (5 ns).
    let built = Model::of("generic.msl").built("PipeBench");
    let seen = built.run(20e-9, 1e-9);
    assert_eq!(Model::data(&built, &seen, "narrow").first(), Some(&200), "the 8-bit instance carries 200");
    assert_eq!(Model::data(&built, &seen, "wide").first(), Some(&3000), "the 16-bit instance carries 3000, which no 8-bit field holds");
}
