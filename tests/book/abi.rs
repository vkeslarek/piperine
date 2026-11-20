//! Scope: chapter 16 of the Book, "Device ABI" (feature `root-e2e`, RE-02 and RE-03): the resistor and
//! the D flip-flop the chapter shows in MSL, compiled and run through `piperine::`, with the values the
//! chapter states about them (two resistors of 1 kOhm and 2 kOhm share one shape, the flip-flop's output
//! starts low and takes its input at the rising edge). The four sentences that describe the interface
//! itself (the status of a call, the thunks, an emulator's clock, a time point) are `prose` rows: they
//! state the design of the ABI crate, not a value or an outcome of a run. Each test opens with the
//! inventory ids it holds, from `tests/book/inventory.tsv`.

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
    // abi#2 (run): `Resistor`, `i(p, n) +> v(p, n) / r`; abi (prose before abi#5): "Two resistors of 1 kΩ
    // and 2 kΩ share this shape": placed in series across a 3 V source, the middle node sits at 2 V
    let chapter = Chapter::read();
    let after = "module Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\nmodule Top() { signal vin: Electrical; signal mid: Electrical; let s = Vdc(vin) { dc: 3.0 }; let a = Resistor(vin, mid) { r: 1k }; let b = Resistor(mid, gnd) { r: 2k }; }\n";
    let ran = chapter.wrapped("resistor.msl", "", "abi#2", after).run(Faulty::operating_point_of("Top")).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.mid") - 2.0).abs() < 1e-9, "3 V * 2k / (1k + 2k): {}", ran.signal("Top.mid"));
    assert!((ran.signal("Top.vin") - 3.0).abs() < 1e-9, "the source holds its node: {}", ran.signal("Top.vin"));
}

#[test]
fn the_flip_flop_starts_low_and_takes_its_input_at_the_rising_edge() {
    // abi#4 (run): `Dff`, `on clk.rise() { q <- d; }` with `out q: Bit = false`: the output starts low and,
    // after the clock rises at 1 us with d high, is high
    let chapter = Chapter::read();
    let after = "module Top() { signal clk: Bit = false; signal d: Bit = true; signal q: Bit; let f = Dff(clk, d, q); }\ndigital for Top { on at(1u) { clk <- true; } }\n";
    let overrides = piperine::Overrides { print_step: Some(1e-6), ..Faulty::transient_of_top(3e-6, None) };
    let ran = chapter.wrapped("dff.msl", "", "abi#4", after).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.cell(0, "Top.q"), &Cell::Bool(false), "q starts low");
    let last = ran.table.rows.len() - 1;
    assert_eq!(ran.cell(last, "Top.q"), &Cell::Bool(true), "q took d at the edge");
}
