//! The four-state `Logic<N>` and the operator traits it is built on, run from MSL source on the Rust backend
//! (feature `explicit-unknowns` phase B, EU-07 to EU-10): one test per run-time criterion, each model written in
//! `tests/logic/`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is the specification's own table (part VII §22.1) written in the test, never read back from the
//! model.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;
#[allow(dead_code)]
#[path = "source/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "source/run.rs"]
mod run;

use piperine_api::Field;
use run::SourceRun;

/// The model files of `tests/logic/`.
const RUN: SourceRun = SourceRun::in_directory("tests/logic");

#[test]
fn an_operator_on_a_library_type_runs_its_impl() {
    // H.10 rules 1 and 4 (EU-08): `a & b`, `a & 6u4`, `!a` and `a &= b` on `Quad` call the functions of the `impl`s the
    // model writes. `Quad` is `binary` then `value`, one byte each. `both` is `{3 | 4, 12 & 10}`, `masked` is
    // `{3, 12 & 6}`, `inverted` is `{3, !12 & 15}`, `compound` is `{3 | 8, 12 & 14}`
    RUN.assert_commits("quad.msl", &["m0", "m1", "m2", "m3"], &[vec![7, 8], vec![3, 4], vec![3, 3], vec![11, 12]]);
}

#[test]
fn logic_lays_out_two_planes() {
    // §22.1.1 rule 1 (EU-07 AC 1, 2): `Logic<4>` is `binary` then `value`, one byte each. `Logic::from(0b1010u4)` is
    // all-binary (`binary` 1111) over `value` 1010; the literal `{ binary: 3, value: 5 }` is itself
    RUN.assert_commits("planes.msl", &["m0", "m1"], &[vec![15, 10], vec![3, 5]]);
}

#[test]
fn at_gives_the_state_of_one_level() {
    // §22.1.1 rule 3 (EU-07 AC 4): `{ binary: 0b0011, value: 0b0101 }` has level 0 `1`, level 1 `0`, level 2 `Z`, level 3 `X`;
    // `LogicState` is `X`, `L`, `H`, `Z` in tag order 0 to 3
    RUN.assert_commits("planes.msl", &["s0", "s1", "s2", "s3"], &[vec![2], vec![1], vec![3], vec![0]]);
}

#[test]
fn logic_reads_a_four_state_pattern() {
    // §22.1.4 rules 2 and 3 (EU-07 AC 3, C25): `logic("10xz")` has levels 3 to 0 equal to 1, 0, X, Z: `binary` 1100 and
    // `value` 1001 (X is `(0, 0)`, Z is `(0, 1)`). `logic("10xz") & Logic::from(0b1111u4)` is the specification's `10xx`
    // (a Z level under `&` with a 1 is X): `binary` 1100, `value` 1000
    RUN.assert_commits("patterns.msl", &["m0", "m1"], &[vec![0b1100, 0b1001], vec![0b1100, 0b1000]]);
    // a pattern with another character or another length is `E0702`, naming what is wrong
    for (file, named) in [("bad_character.msl", "`q`"), ("bad_length.msl", "3 characters")] {
        let refused = RUN.project(file).elaborate("Bench").err().map(|rejection| rejection.to_string()).unwrap_or_default();
        assert!(refused.contains("E0702") && refused.contains(named), "{file}: {refused}");
    }
}

#[test]
fn logic_and_matches_bits_give_the_table_values() {
    // §22.1.4 (EU-07 AC 3): `matches_bits(s, p)` is true when each level of `s` is the state its character names, `x`
    // naming any: `10xz` against `10xz` and `1xxz` and `xxxx` holds, against `0xxz` (level 3 is 1) and `10x0` (level 0 is Z)
    // does not
    RUN.assert_commits("patterns.msl", &["t0", "t1", "t2", "t3", "t4"], &[vec![1], vec![1], vec![0], vec![0], vec![1]]);
}

#[test]
fn a_logic_bit_rises_into_one_and_falls_into_zero() {
    // §22.1.5 (EU-20 AC 3, D9): `l.rise()` is a change that ends at 1 and `l.fall()` a change that ends at 0, wherever it
    // started. `l` steps through 1, 0, Z, 1, X, 0, 1, Z, 0 from X: it rises at steps 1 (from X), 4 (from Z) and 7 (from
    // 0) and falls at steps 2 (from 1), 6 (from X) and 9 (from Z); 0 to Z, 1 to X and 1 to Z are changes that are no edge
    let records = RUN.records_of("edges.msl", "Bench");
    let edges: Vec<(String, Vec<Field>)> = records
        .into_iter()
        .map(|record| (record.stream.to_string(), record.fields.iter().map(|(_, field)| field.clone()).collect()))
        .collect();
    let step = |kind: &str, at: i128| (kind.to_string(), vec![Field::Int(at)]);
    let wanted = [step("rise", 1), step("fall", 2), step("rise", 4), step("fall", 6), step("rise", 7), step("fall", 9)];
    assert_eq!(edges, wanted);
}

/// A `Logic<N>` as its pin holds it (abi.md §3.1): the plane `binary` then the plane `value`, each `N` bits in the
/// smallest of one, two, four or eight bytes, little endian.
struct Pin;

impl Pin {
    /// The characters of the pattern the pin bytes hold, highest level first: `0` is `(1, 0)`, `1` is `(1, 1)`, `x` is
    /// `(0, 0)` and `z` is `(0, 1)` (§22.1.1).
    fn pattern(bytes: &[u8], levels: usize) -> String {
        let plane = bytes.len() / 2;
        // level `k` is bit `k` of a plane, whatever its width: byte `k / 8`, bit `k % 8`
        let bit = |at: usize, level: usize| (bytes[at * plane + level / 8] >> (level % 8)) & 1;
        (0..levels)
            .rev()
            .map(|level| match (bit(0, level), bit(1, level)) {
                (1, 0) => '0',
                (1, 1) => '1',
                (0, 0) => 'x',
                _ => 'z',
            })
            .collect()
    }
}

/// The four-state tables of §22.1.2, per level: each takes two characters of `0`, `1`, `x` and `z` and gives one. An `x`
/// or a `z` operand is X in the tables; a result is never `z`.
struct Tables;

impl Tables {
    /// Whether a level is 0 or 1.
    fn binary(level: char) -> bool {
        level == '0' || level == '1'
    }

    /// `&`: 0 when either is 0, 1 when both are 1, X otherwise.
    fn and(a: char, b: char) -> char {
        match (a, b) {
            ('0', _) | (_, '0') => '0',
            ('1', '1') => '1',
            _ => 'x',
        }
    }

    /// `|`: 1 when either is 1, 0 when both are 0, X otherwise.
    fn or(a: char, b: char) -> char {
        match (a, b) {
            ('1', _) | (_, '1') => '1',
            ('0', '0') => '0',
            _ => 'x',
        }
    }

    /// `^`: the exclusive or of two binary levels, X otherwise.
    fn xor(a: char, b: char) -> char {
        match (Tables::binary(a) && Tables::binary(b), a == b) {
            (true, true) => '0',
            (true, false) => '1',
            _ => 'x',
        }
    }

    /// `!`: 1 for 0, 0 for 1, X for X and for Z.
    fn not(a: char) -> char {
        match a {
            '0' => '1',
            '1' => '0',
            _ => 'x',
        }
    }

    /// The pattern of `operation` applied to each pair of levels of two patterns of one length.
    fn zip(a: &str, b: &str, operation: fn(char, char) -> char) -> String {
        a.chars().zip(b.chars()).map(|(a, b)| operation(a, b)).collect()
    }
}

#[test]
fn and_follows_the_four_state_table() {
    // §22.1.2 table `&` (EU-08 AC 1): `a` is `00001111xxxxzzzz` and `b` cycles `01xz`, so the 16 levels of `a & b` are the 16 pairs
    // of states; each level is the table's entry (a Z operand is X)
    let (a, b) = ("00001111xxxxzzzz", "01xz01xz01xz01xz");
    let got = RUN.first_commits("tables.msl", &["p_and"]);
    assert_eq!(Pin::pattern(&got[0], 16), Tables::zip(a, b, Tables::and), "`&` over the 16 pairs of states");
}

#[test]
fn or_follows_the_four_state_table() {
    // §22.1.2 table `|` (EU-08 AC 2): the same 16 pairs
    let (a, b) = ("00001111xxxxzzzz", "01xz01xz01xz01xz");
    let got = RUN.first_commits("tables.msl", &["p_or"]);
    assert_eq!(Pin::pattern(&got[0], 16), Tables::zip(a, b, Tables::or), "`|` over the 16 pairs of states");
}

#[test]
fn xor_follows_the_four_state_table() {
    // §22.1.2 table `^` (EU-08 AC 3): the same 16 pairs
    let (a, b) = ("00001111xxxxzzzz", "01xz01xz01xz01xz");
    let got = RUN.first_commits("tables.msl", &["p_xor"]);
    assert_eq!(Pin::pattern(&got[0], 16), Tables::zip(a, b, Tables::xor), "`^` over the 16 pairs of states");
}

#[test]
fn not_follows_the_four_state_table() {
    // §22.1.2 table `!` (EU-08 AC 4): 0 gives 1, 1 gives 0, X and Z give X
    let got = RUN.first_commits("tables.msl", &["p_not"]);
    let wanted: String = "00001111xxxxzzzz".chars().map(Tables::not).collect();
    assert_eq!(Pin::pattern(&got[0], 16), wanted);
}

#[test]
fn shifts_move_states_and_fill_with_zero() {
    // §22.1.2 rule 2 (EU-08 AC 5): `10xz1100 << 2` moves every level up by two with its state and shifts in 0 below;
    // `>> 3` moves them down by three and shifts in 0 above: `xz110000` and `00010xz1`
    let got = RUN.first_commits("tables.msl", &["p_shl", "p_shr"]);
    assert_eq!(Pin::pattern(&got[0], 8), "xz110000", "`<<`");
    assert_eq!(Pin::pattern(&got[1], 8), "00010xz1", "`>>`");
}

#[test]
fn a_logic_and_a_binary_value_combine_level_by_level() {
    // §22.1.2 rule 3 (EU-08 AC 6): `Logic<4> & u4`, `u4 | Logic<4>`, `Logic<4> ^ u4`, `Logic ^ bool`, `bool & Logic` and
    // `Logic | bool` give the table's `Logic`, the binary operand converted level by level. `10xz & 1100` is `1000`,
    // `0101 | 10xz` is `11x1`, `10xz ^ 1111` is `01xx`, `1 ^ 0` is `1`, `true & x` is `x`, `z | true` is `1`
    let got = RUN.first_commits("tables.msl", &["m0", "m1", "m2", "m3", "m4", "m5"]);
    let wanted = [("1000", 4), ("11x1", 4), ("01xx", 4), ("1", 1), ("x", 1), ("1", 1)];
    for (k, (pattern, levels)) in wanted.iter().enumerate() {
        assert_eq!(Pin::pattern(&got[k], *levels), *pattern, "m{k}");
    }
}

#[test]
fn equality_of_logic_is_a_logic() {
    // §22.1.2 rule 4 (EU-09 AC 1): `==` is X when any level of either operand is X or Z, else 1 when every level is equal
    // and 0 when not; `!=` is its `!`. `1010 == 1010` is `1`, `1010 == 1000` is `0`, `10xz == 10xz` and `1010 == 101z` are
    // `x`; `!=` gives `0`, `1`, `x`
    let got = RUN.first_commits("tables.msl", &["e0", "e1", "e2", "e3", "n0", "n1", "n2"]);
    let wanted = ["1", "0", "x", "x", "0", "1", "x"];
    for (k, pattern) in wanted.iter().enumerate() {
        assert_eq!(Pin::pattern(&got[k], 1), *pattern, "comparison {k}");
    }
}

#[test]
fn same_compares_the_four_states() {
    // §22.1.4 rule 1 (EU-09 AC 2): `same` is true exactly when every level holds the same one of the four states in both:
    // `10xz` with itself, `zzzz` with itself; false for `10xz` against `10xx` and for `xxxx` against `zzzz`
    let got = RUN.first_commits("tables.msl", &["s0", "s1", "s2", "s3"]);
    assert_eq!(got, [vec![1], vec![0], vec![1], vec![0]]);
}

#[test]
fn the_four_tests_of_a_logic() {
    // §22.1.1 rule 4 (EU-09 AC 3): `is_high`, `is_low`, `is_z` and `is_x` are true exactly when every level is 1, 0, Z or X
    // (`t0` to `t3`: all and not all; `t4` to `t7` likewise), and a mixed value or another uniform one is none of them
    let names: Vec<String> = (0..16).map(|k| format!("t{k}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let got = RUN.first_commits("tables.msl", &names);
    let wanted = [true, false, true, false, true, false, true, false, false, false, false, false, false, false, false, false];
    for (k, wanted) in wanted.iter().enumerate() {
        assert_eq!(got[k], vec![u8::from(*wanted)], "t{k}");
    }
}

#[test]
fn a_binary_value_converts_to_logic() {
    // §22.1.1 rule 6 (EU-10 AC 1): `Logic::from(0b1010u4)` is `1010`, `Logic::from(true)` is `1`, `Logic::from(false)` is `0`
    let got = RUN.first_commits("tables.msl", &["c0", "c1", "c2"]);
    assert_eq!((Pin::pattern(&got[0], 4), Pin::pattern(&got[1], 1), Pin::pattern(&got[2], 1)), ("1010".to_string(), "1".to_string(), "0".to_string()));
}

#[test]
fn a_logic_converts_to_an_option_of_bits() {
    // §22.1.1 rule 5 (EU-10 AC 2): `to_bits()` is `Some` for all-binary levels (`1010` gives 10) and `None` with an X or a Z
    // (`10xz` gives the fallback 15); `to_bit()` of one level is `Some(true)`, `Some(false)` and `None` for z
    let got = RUN.first_commits("tables.msl", &["o0", "o1", "p0", "p1", "q0", "q1", "q2"]);
    assert_eq!(got, [vec![10], vec![15], vec![1], vec![0], vec![1], vec![0], vec![1]]);
}

/// The volts of each node of `analog.msl` at the operating point.
const NODES: [&str; 7] = ["lit", "dark", "unknown", "floating", "gate_on", "gate_off", "gate_unknown"];

#[test]
fn analog_reads_a_logic_through_a_function() {
    // §3.3.6 (EU-03 AC 2, C9): `analog` reads a `Logic` carrier through a function that returns a `bool`: `is_high()` is true
    // for a level 1 only, so the gate forces 2 V for `H` and 0 V for `L` and for a carrier that never held a value (X)
    let volts = RUN.operating_point("analog.msl", "Bench", &NODES);
    for (name, wanted) in [("gate_on", 2.0), ("gate_off", 0.0), ("gate_unknown", 0.0)] {
        let got = volts[NODES.iter().position(|n| *n == name).unwrap()];
        assert!((got - wanted).abs() < 1e-9, "{name}: {got} V, wanted {wanted}");
    }
}

#[test]
fn to_volts_gives_the_level_of_each_state() {
    // §22.1.1 rule 5 (EU-10 AC 3, C40): `to_volts(1.8)` is 1.8 for 1, 0.0 for 0 and 0.9 (half) for X and for Z, and an `analog`
    // block drives a node from it
    let volts = RUN.operating_point("analog.msl", "Bench", &NODES);
    for (name, wanted) in [("lit", 1.8), ("dark", 0.0), ("unknown", 0.9), ("floating", 0.9)] {
        let got = volts[NODES.iter().position(|n| *n == name).unwrap()];
        assert!((got - wanted).abs() < 1e-9, "{name}: {got} V, wanted {wanted}");
    }
}

#[test]
fn an_option_of_logic_is_none_or_a_whole_vector() {
    // §3.3.3, §3.3.4 (EU-06 AC 3, C22): a carrier of `Option<Logic<4>>` starts at `None` and holds `Some` of four four-state
    // levels once committed. `o` is never driven, so it reads `None` (1) at every edge; `p` reads `None` at the edge of its own
    // commit and `Some` (2) at every later one, with `unwrap_or` giving `10xz`: `binary` 1100, `value` 1001
    let seen: Vec<Vec<Field>> = RUN.records_of("options.msl", "Bench").into_iter().filter(|record| record.stream == "seen").map(|record| record.fields.iter().map(|(_, field)| field.clone()).collect()).collect();
    assert!(seen.len() > 1, "more than one rising edge");
    assert_eq!(seen[0], [Field::Int(1), Field::Int(1), Field::Int(0b0000), Field::Int(0b1111)], "`None` of `p` reads the fallback `zzzz`");
    for record in &seen[1..] {
        assert_eq!(*record, [Field::Int(1), Field::Int(2), Field::Int(0b1100), Field::Int(0b1001)], "`Some(10xz)` after the commit");
    }
}

#[test]
fn the_tables_hold_at_sixty_four_levels() {
    // §22.1.2 (EU-08, N of 64): the same tables at the widest width the planes support: `a` cycles `01xz` and `b` cycles `1xz0`
    let (a, b) = ("01xz".repeat(16), "1xz0".repeat(16));
    let got = RUN.first_commits("wide.msl", &["w_and", "w_or", "w_xor", "w_not", "w_shl"]);
    assert_eq!(Pin::pattern(&got[0], 64), Tables::zip(&a, &b, Tables::and), "`&`");
    assert_eq!(Pin::pattern(&got[1], 64), Tables::zip(&a, &b, Tables::or), "`|`");
    assert_eq!(Pin::pattern(&got[2], 64), Tables::zip(&a, &b, Tables::xor), "`^`");
    assert_eq!(Pin::pattern(&got[3], 64), a.chars().map(Tables::not).collect::<String>(), "`!`");
    assert_eq!(Pin::pattern(&got[4], 64), format!("{}000", &a[3..]), "`<< 3`");
}

#[test]
fn a_logic_of_a_hundred_levels_runs() {
    // §22.1.1, §22.1.2 (EU-07, N of 100; composite-values CV-32, supersedes the refusal naming `GAPS.md` L13): the planes are two
    // `UInt<100>`, so the tables hold level by level at 100 levels as at 64: `a` cycles `01xz` and `b` cycles `1xz0`
    let (a, b) = ("01xz".repeat(25), "1xz0".repeat(25));
    let got = RUN.first_commits("hundred.msl", &["w_and", "w_or", "w_xor", "w_not", "w_shl"]);
    assert_eq!(Pin::pattern(&got[0], 100), Tables::zip(&a, &b, Tables::and), "`&`");
    assert_eq!(Pin::pattern(&got[1], 100), Tables::zip(&a, &b, Tables::or), "`|`");
    assert_eq!(Pin::pattern(&got[2], 100), Tables::zip(&a, &b, Tables::xor), "`^`");
    assert_eq!(Pin::pattern(&got[3], 100), a.chars().map(Tables::not).collect::<String>(), "`!`");
    assert_eq!(Pin::pattern(&got[4], 100), format!("{}000", &a[3..]), "`<< 3`");
}

#[test]
fn an_inferred_logic_of_seventy_levels_runs() {
    // §22.1.4 (finding 60; supersedes the refusal naming `GAPS.md` L13): `logic("<70 ones>")` infers a `Logic<70>`, which
    // elaborates like a written one, and `is_high` of 70 ones is true
    let project = RUN.project("inferred_wide.msl");
    project.elaborate("Bench").unwrap_or_else(|rejection| panic!("{rejection}"));
}

#[test]
fn the_plan_lays_a_logic_pin_as_two_planes() {
    // EU-16 AC 5 (C69): the plan lays the elaborated `Logic<4>` and `Logic<64>` as the struct each is: `binary` at 0 then
    // `value`, each the leaf of `UInt<N>`; sizes 2 and 16 and alignment 1 (the abi's own rule is held by `layouts::a_logic_pin_is_two_planes`)
    use piperine_abi::raw::leaf;
    use piperine_ir::ValueType;
    use piperine_plan::PinLayout;
    let narrow = RUN.project("planes.msl").elaborate("Bench").unwrap_or_else(|rejection| panic!("{rejection}"));
    let wide = RUN.project("wide.msl").elaborate("Bench").unwrap_or_else(|rejection| panic!("{rejection}"));
    for (ir, name, levels, plane) in [(&narrow, "Logic<4>", 4u32, leaf::U8), (&wide, "Logic<64>", 64, leaf::U64)] {
        let index = ir.types().iter().position(|data| data.name == name).unwrap_or_else(|| panic!("the design holds no `{name}`"));
        let layout = PinLayout::in_ir(ir, &ValueType::Struct(piperine_ir::TypeId(index as u32))).expect("a layout");
        let plane_size = levels.div_ceil(8).next_power_of_two();
        let leaves: Vec<(String, u32, u32)> = layout.fields().iter().map(|f| (f.name.as_str().to_string(), f.leaf, f.offset)).collect();
        assert_eq!(leaves, [("binary".to_string(), plane, 0), ("value".to_string(), plane, plane_size)], "{name}");
        assert_eq!(layout.size(), 2 * plane_size, "{name}");
        assert_eq!(layout.align(), 1, "{name}: a composite pin is packed, alignment 1 (EU-16 AC 5 as amended)");
    }
}
