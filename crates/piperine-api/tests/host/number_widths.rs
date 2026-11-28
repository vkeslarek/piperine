//! The number widths at the host (composite-values phase D, CV-29 to CV-32, user Q2 (c)): a value of type `f16`, `f32`, `f128`
//! or an integer wider than 64 bits is one exact cell, never limbs and never an `f64`; its text is the shortest decimal that
//! reads back (floats) or the exact decimal (integers); a param of the type is set from text read in the type's own format and
//! shows what it was set to. The expected texts are the values themselves (`0.1` in each format, `2^199 + 1`), written out
//! by hand from the rules cited.
#![cfg(unix)]

use std::fs;
use std::path::PathBuf;

use piperine_api::{Cell, Failure, FloatCell, FloatFormat, Overrides, Project, Session, WideCell};
use piperine_ir::ValueType;
use piperine_transpile::Settings;

/// A design with a param of each new width.
const PARAMS: &str = "pub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\
    module Leaf(p: Electrical) { a: f32 = 1.5; h: f16 = 0.5; q: f128 = 0.1; w: UInt<200> = 3; s: Int<100> = -7; }\n\
    analog for Leaf { i(p, gnd) +> v(p) / 1k; }\n\
    module Top() { signal x: Electrical; let v1 = Vsource(x, gnd) { dc: 1.0 }; let l = Leaf(x); }\n";

/// The helpers of this target.
struct Host;

impl Host {
    /// A Rust-backend operating-point session of `PARAMS`, in a cache of its own.
    fn session() -> (Session, tempfile::TempDir) {
        let cache = tempfile::tempdir().expect("a cache");
        let dir = tempfile::tempdir().expect("a source directory");
        let file: PathBuf = dir.path().join("top.msl");
        fs::write(&file, PARAMS).expect("the source");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { analysis: Some("op".into()), backend: Some("rust".into()), top: Some("Top".into()), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let session = Session::open(&project, &spec, &settings).unwrap_or_else(|f| panic!("{f}"));
        (session, cache)
    }

    /// The cell the session shows for `Top.l.<param>`.
    fn shown(session: &Session, param: &str) -> Cell {
        session.metadata().params.into_iter().find(|p| p.path == format!("Top.l.{param}")).unwrap_or_else(|| panic!("no param `{param}`")).value
    }

    /// The text of a float cell.
    fn float_text(cell: &Cell) -> String {
        match cell {
            Cell::Float(float) => float.text(),
            other => panic!("a float cell, found {other:?}"),
        }
    }

    /// The text of a wide cell.
    fn wide_text(cell: &Cell) -> String {
        match cell {
            Cell::Wide(wide) => wide.text(),
            other => panic!("a wide cell, found {other:?}"),
        }
    }
}

#[test]
fn the_host_reads_each_width_as_one_exact_value() {
    // CV-32 AC 4 (§2 table, abi.md §3.1 P3): a pin of `UInt<200>` holding 2^199 + 1 is one integer, never its four limbs
    let mut bytes = vec![0u8; 32];
    bytes[0] = 1;
    bytes[24] = 0x80;
    let cell = Cell::decode(&ValueType::UInt(200), &bytes).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::wide_text(&cell), "803469022129495137770981046170581301261101496891396417650689", "2^199 + 1 exactly");
    assert!(matches!(&cell, Cell::Wide(wide) if wide.width == 200 && !wide.signed && wide.limbs == vec![1, 0, 0, 0x80]));
    // an `Int<100>` of -2 is two's complement over two limbs with its sign carried to the limb boundary, read back as -2
    let negative = Cell::decode(&ValueType::Int(100), &[0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::wide_text(&negative), "-2");
    assert_eq!(Cell::decode(&ValueType::Int(100), &[0; 8]).map(|_| ()).unwrap_err().to_string().contains("8 bytes"), true, "a length other than the layout's is a refusal");

    // the shortest decimal that reads back, in each format (QD6): 0.1 is `0.1` in `f32`, `f16` and `f128` alike, each from its own bits
    let single = Cell::decode(&ValueType::F32, &0x3dcc_cccdu32.to_le_bytes()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::float_text(&single), "0.1");
    let half = Cell::decode(&ValueType::F16, &0x2e66u16.to_le_bytes()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::float_text(&half), "0.1");
    let quad = Cell::decode(&ValueType::F128, &0x3ffb_9999_9999_9999_9999_9999_9999_999au128.to_le_bytes()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::float_text(&quad), "0.1");
    assert_eq!(Cell::Float(FloatCell { format: FloatFormat::Single, bits: 0x3dcc_cccd }), single);

    // the plot takes the nearest `f64`: the `f32` 0.1 is 0.10000000149011612
    match &single {
        Cell::Float(float) => assert_eq!(float.nearest_f64(), 0.100_000_001_490_116_12),
        other => panic!("{other:?}"),
    }
    match &cell {
        Cell::Wide(wide) => assert_eq!(wide.nearest_f64(), 2f64.powi(199)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_param_of_each_width_shows_its_default_and_is_set_by_text_in_its_own_format() {
    // A.8 rule 8, §3.2: a param of a number width shows its exact default, a `--param` text is read in the param's own format
    // (never through an `f64`) and reads back as the same value, and a text that is no value of the type refuses (`E0802`) and changes nothing
    let (mut session, _cache) = Host::session();
    assert_eq!(Host::float_text(&Host::shown(&session, "a")), "1.5");
    assert_eq!(Host::float_text(&Host::shown(&session, "h")), "0.5");
    assert_eq!(Host::float_text(&Host::shown(&session, "q")), "0.1");
    assert_eq!(Host::wide_text(&Host::shown(&session, "w")), "3");
    assert_eq!(Host::wide_text(&Host::shown(&session, "s")), "-7");

    session.set_text("Top.l.a", "0.1").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::shown(&session, "a"), Cell::Float(FloatCell { format: FloatFormat::Single, bits: 0x3dcc_cccd }), "the nearest f32 to 0.1");
    session.set_text("Top.l.q", "0.3").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::float_text(&Host::shown(&session, "q")), "0.3");
    assert!(matches!(Host::shown(&session, "q"), Cell::Float(FloatCell { format: FloatFormat::Quad, bits }) if bits == 0x3ffd_3333_3333_3333_3333_3333_3333_3333), "the nearest f128 to 0.3 is 0x3ffd333333333333 3333333333333333");
    session.set_text("Top.l.h", "65504").unwrap_or_else(|f| panic!("{f}"));
    // the largest `f16` is 65504, whose shortest text that reads back in `f16` is `65500` (the neighbours are 32 apart)
    assert_eq!(Host::shown(&session, "h"), Cell::Float(FloatCell { format: FloatFormat::Half, bits: 0x7bff }));
    assert_eq!(Host::float_text(&Host::shown(&session, "h")), "65500");
    session.set_text("Top.l.w", "0x8000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0001").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::wide_text(&Host::shown(&session, "w")), "3138550867693340381917894711603833208051177722232017256449", "2^191 + 1 (48 hex digits, the top bit set)");
    session.set_text("Top.l.s", "-123456789012345678901234567").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Host::wide_text(&Host::shown(&session, "s")), "-123456789012345678901234567");
    assert_eq!(WideCell::parse(true, 100, "-5").map(|c| c.text()), Some("-5".to_string()));

    for (param, text) in [("Top.l.h", "70000"), ("Top.l.a", "six"), ("Top.l.s", "0x8000000000000000000000000"), ("Top.l.w", "-1"), ("Top.l.w", "1.5")] {
        match session.set_text(param, text) {
            Err(Failure::Host { code: "E0802", message }) => assert!(message.contains(param) && message.contains(text), "{message}"),
            other => panic!("`{param}` = `{text}`: E0802, found {other:?}"),
        }
    }
    assert_eq!(Host::shown(&session, "h"), Cell::Float(FloatCell { format: FloatFormat::Half, bits: 0x7bff }), "a refused set changes nothing");
}
