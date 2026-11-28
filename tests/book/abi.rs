//! Scope: chapter 18 of the Book, "The device ABI" (feature `root-e2e`, RE-02 and RE-03): the resistor and
//! the D flip-flop the chapter shows in MSL, compiled and run through `piperine::`, with the values the
//! chapter states about them (a divider of 1 kOhm and 2 kOhm across 3 V holds 2 V, the flip-flop's output
//! starts low and takes its input at the rising edge). The chapter and chapter 19, "Writing a device",
//! teach the device ABI that the feature `device-abi-simplification` implements: their `rust` and `text`
//! listings are `shown` rows and their other sentences `prose` rows under the gap X10 of `GAPS.md`, which
//! that feature closes by compiling and running each one. Each test opens with the inventory ids it
//! holds, from `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod faulty;
    pub mod inventory;
    pub mod model;
    pub mod records;
    pub mod refusal;
}

use piperine::Cell;
use support::chapter::Chapter;
use support::faulty::Faulty;

#[test]
fn two_resistors_of_one_shape_divide_a_source_by_their_values() {
    // abi#1 (run): `Resistor`, `i(p, n) +> v(p, n) / r`; abi~f4c47309 (holds): "Run the operating point
    // and the middle node sits at 2 V": 1 kΩ and 2 kΩ in series across a 3 V source
    let chapter = Chapter::read();
    let after = "module Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\nmodule Top() { signal vin: Electrical; signal mid: Electrical; let s = Vdc(vin) { dc: 3.0 }; let a = Resistor(vin, mid) { r: 1k }; let b = Resistor(mid, gnd) { r: 2k }; }\n";
    let ran = chapter.wrapped("resistor.msl", "", "abi#1", after).run(Faulty::operating_point_of("Top")).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.mid") - 2.0).abs() < 1e-9, "3 V * 2k / (1k + 2k): {}", ran.signal("Top.mid"));
    assert!((ran.signal("Top.vin") - 3.0).abs() < 1e-9, "the source holds its node: {}", ran.signal("Top.vin"));
}

#[test]
fn the_flip_flop_starts_low_and_takes_its_input_at_the_rising_edge() {
    // abi#3 (run): `Dff`, `on clk.rise() { q <- d; }` with `out q: Bit = false`; abi~a5680dbb (holds): "q
    // starts low, and after the edge it is high": the clock rises at 1 us with d high
    let chapter = Chapter::read();
    let after = "module Top() { signal clk: Bit = false; signal d: Bit = true; signal q: Bit; let f = Dff(clk, d, q); }\ndigital for Top { on at(1u) { clk <- true; } }\n";
    let overrides = piperine::Overrides { print_step: Some(1e-6), ..Faulty::transient_of_top(3e-6, None) };
    let ran = chapter.wrapped("dff.msl", "", "abi#3", after).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.cell(0, "Top.q"), &Cell::Bool(false), "q starts low");
    let last = ran.table.rows.len() - 1;
    assert_eq!(ran.cell(last, "Top.q"), &Cell::Bool(true), "q took d at the edge");
}

#[test]
fn the_abi_chapter_describes_the_unreleased_contract() {
    // EU-18 AC 6: the chapter says the contract is still major 1 and unreleased (it changes in place until the
    // first release), and describes the layouts without a flag byte: `BIT` one `BOOL`, `Option` a presence
    // field and a payload, `Logic<N>` two planes, a drive pin with a plain `driving` byte, one `commit`
    let book = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/book/content.html")).expect("the Book");
    let start = book.find("<section id=\"abi\"").expect("chapter abi");
    let chapter = &book[start..start + book[start..].find("</section>").expect("a chapter closes")];
    for fact in ["<strong>major 1</strong> and is <strong>unreleased</strong>", "it changes in place", "<code>v: BOOL</code> at offset 0: size 1, alignment 1", "A <code>BOOL</code> presence field, then the layout of <code>T</code>", "Two fields of the leaf of <code>UInt&lt;N&gt;</code>", "<code>driving: BOOL</code>", "There is one <code>commit</code>, and a field is <code>{ name, leaf, offset, count }</code>"] {
        assert!(chapter.contains(fact), "the ABI chapter lacks {fact}");
    }
    for gone in ["known byte", "commit_with", "HAS_KNOWN", "known: 1"] {
        assert!(!chapter.contains(gone), "the ABI chapter still says {gone}");
    }
}
