//! Scope: the host writes a four-state pattern (L71, §22.1.4, `docs/spec/part_viii_host.md` §23.1): `Session::drive_pattern` drives
//! a top's `in` port of type `Logic<N>` and the host's drive of a top's resolved port whose drive type is `Logic<N>`, with the
//! text the host prints a cell as (`10xz`, highest level first). A character outside `0 1 x z` and a length other than `N` are
//! `E0702`; a port that holds no `Logic` is `E0802`. A `Logic<N>` param (§8.1.15) is set the same way, by the pattern as text
//! (`Session::set_text`, `--param Top.pat=01zx`) or by its plane leaves (`Top.pat.binary`), exactly at any `N` (60 and 100 levels
//! included: no plane goes through an `f64`).
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Cell, Failure, LogicPattern, Overrides, Project, Session, Table};
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::sinks::Ignore;

/// The designs and runs of this target.
struct Bench;

impl Bench {
    /// A top with a four-state `in` port read by a signal, a resolved port whose drive type is `Logic<4>` (it resolves to the last
    /// drive, `zzzz` when nobody drives it) and an `in` port of another type.
    const DESIGN: &str = "\
    nature Bus { value: Logic<4> }
    impl Resolve<Logic<4>> for Bus {
        fn resolve<const N: usize>(drives: [Logic<4>; N]) -> Logic<4> {
            drives.iter().fold(logic(\"zzzz\"), |held, drive| drive)
        }
    }
    module Top(in a: Bit<Logic<4>> = logic(\"0000\"), in byte: Bit<u8> = 0, w: Bus) {
        pat: Logic<4> = logic(\"10xz\");
        signal q: Bit<Logic<4>>;
        signal held: Bit<Logic<4>>;
        q = a;
        held = pat;
    }
    ";

    /// A top with two wide `Logic` params, of 60 and of 100 levels (planes no `f64` holds exactly, the second wider than 64
    /// bits), each read by a signal.
    const WIDE: &str = "\
    module Top() {
        narrow: Logic<60> = Logic { binary: 0, value: 0 };
        wide: Logic<100> = Logic { binary: 0, value: 0 };
        signal held_narrow: Bit<Logic<60>>;
        signal held_wide: Bit<Logic<100>>;
        held_narrow = narrow;
        held_wide = wide;
    }
    ";

    /// A session of `Bench::DESIGN` for `analysis`.
    pub fn session(analysis: &str, extra: Overrides) -> Session {
        Bench::session_of(Bench::DESIGN, analysis, extra)
    }

    /// A session of `design` for `analysis`.
    pub fn session_of(design: &str, analysis: &str, extra: Overrides) -> Session {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, design).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { backend: Some("rust".into()), top: Some("Top".into()), analysis: Some(analysis.into()), ..extra };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"))
    }

    /// The operating point of a session, as a table.
    pub fn op(session: &mut Session) -> Table {
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"))
    }

    /// The cell of the signal `name` of an operating point table.
    pub fn cell<'a>(table: &'a Table, name: &str) -> &'a Cell {
        &table.rows.iter().find(|row| row[0] == Cell::Text(name.to_string())).unwrap_or_else(|| panic!("no signal {name} in {table:?}")).get(1).expect("a value")
    }

    /// A pattern of `levels` levels using all four states, no `f64` able to hold either of its planes once `levels` passes 53:
    /// `1`, `z`, `x`, then `0` and `1` alternating, and `1` as the lowest level.
    pub fn wide_pattern(levels: usize) -> String {
        let middle: String = (0..levels - 4).map(|k| if k % 2 == 0 { '0' } else { '1' }).collect();
        format!("1zx{middle}1")
    }

    /// The text of the failure's code and message.
    pub fn refusal(failure: Failure) -> (&'static str, String) {
        match failure {
            Failure::Host { code, message } => (code, message),
            other => panic!("a host failure, found {other}"),
        }
    }
}

#[test]
fn a_pattern_lays_out_its_two_planes_highest_level_first() {
    // §22.1.4 rule 3, abi.md P3: `10xz` is levels 3 to 0 equal to 1, 0, X, Z; `binary` has the levels that are 0 or 1 (levels 3
    // and 2), `value` the levels that are 1 or Z (levels 3 and 0)
    let pattern = LogicPattern::parse("10xz", 4, "a test").expect("a pattern");
    assert_eq!(pattern.bytes(1), vec![0b1100, 0b1001]);
    // nine levels take two bytes a plane: level 8 is `1` (binary and value, bit 0 of byte 1) and levels 7 to 0 are `0` (binary only)
    let wide = LogicPattern::parse(&format!("1{}", "0".repeat(8)), 9, "a test").expect("a pattern");
    assert_eq!(wide.bytes(2), vec![0xff, 0x01, 0x00, 0x01]);
}

#[test]
fn a_bad_character_and_a_wrong_length_are_e0702() {
    // §22.1.4 rule 2: any other character, or a length other than `N`, is `E0702`
    for (text, wanted) in [("10xq", "`q`"), ("10X1", "`X`"), ("10x", "3 levels"), ("10xz1", "5 levels"), ("", "0 levels")] {
        let failure = LogicPattern::parse(text, 4, "the port `a`").expect_err(text);
        let (code, message) = Bench::refusal(failure);
        assert_eq!(code, "E0702", "{text}");
        assert!(message.contains(wanted) && message.contains("the port `a`"), "{text}: {message}");
    }
}

#[test]
fn the_host_drives_a_four_state_in_port() {
    // the pattern is the port's value from the start: the signal that reads it holds the same four states, and the port
    // prints as the pattern (EU-15 AC 4)
    let mut s = Bench::session("op", Overrides::default());
    s.drive_pattern("a", "10xz", 0.0).expect("a pattern of 4 levels");
    let table = Bench::op(&mut s);
    assert_eq!(Bench::cell(&table, "Top.a"), &Cell::Text("10xz".to_string()));
    assert_eq!(Bench::cell(&table, "Top.q"), &Cell::Text("10xz".to_string()));
}

#[test]
fn the_host_drive_of_a_resolved_port_is_the_net_when_it_is_the_only_drive() {
    // the host holds the first drive of a top's resolved port (§4 rule 2): with nobody else driving it, the net is the host's
    // pattern; undriven it is the rule's `zzzz`
    let mut driven = Bench::session("op", Overrides::default());
    driven.drive_pattern("w", "01xz", 0.0).expect("the host's drive");
    assert_eq!(Bench::cell(&Bench::op(&mut driven), "Top.w"), &Cell::Text("01xz".to_string()));
    let mut alone = Bench::session("op", Overrides::default());
    assert_eq!(Bench::cell(&Bench::op(&mut alone), "Top.w"), &Cell::Text("zzzz".to_string()));
}

#[test]
fn a_timed_pattern_lands_at_its_time() {
    // §23.1 rule 4: made during the run, a drive is a commit due at its time: the port holds the initial value before it and the
    // pattern after it
    let mut s = Bench::session("transient", Overrides { stop: Some(2e-6), print_step: Some(1e-6), ..Overrides::default() });
    s.drive_pattern("a", "1111", 1e-6).expect("a timed pattern");
    let table = s.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
    let column = table.columns.iter().position(|name| name == "Top.a").expect("the port's column");
    assert_eq!(table.rows.first().map(|row| row[column].clone()), Some(Cell::Text("0000".to_string())), "{table:?}");
    assert_eq!(table.rows.last().map(|row| row[column].clone()), Some(Cell::Text("1111".to_string())), "{table:?}");
}

#[test]
fn a_refused_pattern_changes_nothing() {
    // `E0702` for a bad pattern, `E0802` for a port with no `Logic`, `E0804` for a signal and `E0801` for a name nobody has; the
    // session keeps the port's initial value
    let mut s = Bench::session("op", Overrides::default());
    assert_eq!(Bench::refusal(s.drive_pattern("a", "10xq", 0.0).expect_err("a bad character")).0, "E0702");
    assert_eq!(Bench::refusal(s.drive_pattern("a", "10x", 0.0).expect_err("a wrong length")).0, "E0702");
    let (code, message) = Bench::refusal(s.drive_pattern("byte", "1010", 0.0).expect_err("a u8"));
    assert!(code == "E0802" && message.contains("`byte`"), "{code}: {message}");
    assert_eq!(Bench::refusal(s.drive_pattern("q", "1010", 0.0).expect_err("a signal")).0, "E0804");
    assert_eq!(Bench::refusal(s.drive_pattern("nothing", "1010", 0.0).expect_err("no such port")).0, "E0801");
    assert_eq!(Bench::cell(&Bench::op(&mut s), "Top.a"), &Cell::Text("0000".to_string()));
}

#[test]
fn a_logic_param_is_set_by_its_pattern() {
    // §8.1.15 and §22.1.4: `pat: Logic<4> = logic("10xz")` is a param; its text is the pattern the host prints a cell as. The
    // default reads back `10xz`, `01zx` replaces it, and a bad pattern is `E0702` and changes nothing
    let mut s = Bench::session("op", Overrides::default());
    s.set_text("Top.pat", "01zx").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Bench::cell(&Bench::op(&mut s), "Top.held"), &Cell::Text("01zx".to_string()));
    let mut untouched = Bench::session("op", Overrides::default());
    assert_eq!(Bench::refusal(untouched.set_text("Top.pat", "01zq").expect_err("a bad character")).0, "E0702");
    assert_eq!(Bench::refusal(untouched.set_text("Top.pat", "01z").expect_err("a wrong length")).0, "E0702");
    assert_eq!(Bench::cell(&Bench::op(&mut untouched), "Top.held"), &Cell::Text("10xz".to_string()));
}

#[test]
fn a_logic_param_of_any_width_is_set_exactly_by_its_pattern() {
    // §23.1 rule 13: a `Logic<N>` param is written as its pattern for every `N`, its planes written whole, never through an `f64`:
    // `Logic<60>` (planes of one 64-bit word, above 2^53) and `Logic<100>` (planes of two words) read back the very pattern, by
    // `Session::set_text` and by `--param`
    let (narrow, wide) = (Bench::wide_pattern(60), Bench::wide_pattern(100));
    let mut by_session = Bench::session_of(Bench::WIDE, "op", Overrides::default());
    by_session.set_text("Top.narrow", &narrow).unwrap_or_else(|f| panic!("{f}"));
    by_session.set_text("Top.wide", &wide).unwrap_or_else(|f| panic!("{f}"));
    let table = Bench::op(&mut by_session);
    assert_eq!(Bench::cell(&table, "Top.held_narrow"), &Cell::Text(narrow.clone()));
    assert_eq!(Bench::cell(&table, "Top.held_wide"), &Cell::Text(wide.clone()));
    let texts = vec![("Top.narrow".to_string(), narrow.clone()), ("Top.wide".to_string(), wide.clone())];
    let mut by_flag = Bench::session_of(Bench::WIDE, "op", Overrides { param_texts: texts, ..Overrides::default() });
    let table = Bench::op(&mut by_flag);
    assert_eq!(Bench::cell(&table, "Top.held_narrow"), &Cell::Text(narrow));
    assert_eq!(Bench::cell(&table, "Top.held_wide"), &Cell::Text(wide));
}

#[test]
fn a_logic_param_of_any_width_is_set_exactly_by_its_leaves() {
    // §23.1 rule 13: a plane leaf is an integer written exactly (no `f64` round trip), at 60 and at 100 levels. `binary` all ones
    // and `value` with only its highest, its 54th and its lowest level set: the pattern is `1`, then `0`s with a `1` at level 53
    let narrow_value = (1u128 << 59) | (1 << 53) | 1;
    let wide_value = (1u128 << 99) | (1 << 53) | 1;
    let texts = vec![
        ("Top.narrow.binary".to_string(), ((1u128 << 60) - 1).to_string()),
        ("Top.narrow.value".to_string(), narrow_value.to_string()),
        ("Top.wide.binary".to_string(), format!("0x{:x}", (1u128 << 100) - 1)),
        ("Top.wide.value".to_string(), wide_value.to_string()),
    ];
    let pattern = |levels: usize| (0..levels).rev().map(|level| if [levels - 1, 53, 0].contains(&level) { '1' } else { '0' }).collect::<String>();
    let mut s = Bench::session_of(Bench::WIDE, "op", Overrides { param_texts: texts, ..Overrides::default() });
    let table = Bench::op(&mut s);
    assert_eq!(Bench::cell(&table, "Top.held_narrow"), &Cell::Text(pattern(60)));
    assert_eq!(Bench::cell(&table, "Top.held_wide"), &Cell::Text(pattern(100)));
    // a number given for a leaf of the second word lands in it, and one outside the plane's levels is refused
    let mut by_number = Bench::session_of(Bench::WIDE, "op", Overrides::default());
    by_number.set_at("Top.wide.binary", 2f64.powi(99), 0.0).unwrap_or_else(|f| panic!("{f}"));
    by_number.set_at("Top.wide.value", 2f64.powi(99), 0.0).unwrap_or_else(|f| panic!("{f}"));
    let highest = format!("1{}", "x".repeat(99));
    assert_eq!(Bench::cell(&Bench::op(&mut by_number), "Top.held_wide"), &Cell::Text(highest));
    let mut refused = Bench::session_of(Bench::WIDE, "op", Overrides::default());
    assert_eq!(Bench::refusal(refused.set_at("Top.narrow.value", 2f64.powi(60), 0.0).expect_err("2^60 is no 60-level plane")).0, "E0802");
}
