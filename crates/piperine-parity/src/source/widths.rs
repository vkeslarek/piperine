//! The models of the `source_number_widths` target (composite-values phase D): a table of cases,
//! each one expression over the inputs of a bench and the bytes Rust or the exact oracle gives for
//! it, written as the model text `tests/number_widths/<file>` (checked against the committed file)
//! and run at the first rising edge of the bench's oscillator, where every carrier of the table
//! commits its case.

use std::path::Path;

use super::run::SourceRun;

/// One row of a table: `name <- expression`, a carrier of type `carrier_type` that must hold `expected` after the first edge.
pub struct WidthCase {
    /// The carrier's name.
    pub name: String,
    /// The MSL type of the value (`f32`, `bool`, `UInt<200>`).
    pub carrier_type: String,
    /// The value the carrier starts at, as MSL text.
    pub zero: String,
    /// The MSL expression the reaction commits.
    pub expression: String,
    /// The bytes of the carrier's committed value (abi.md §3.1: little-endian, wide values as `U64` limbs).
    pub expected: Vec<u8>,
}

/// A bench: inputs the cases read, consts the cases fold, and the cases.
pub struct WidthBench {
    /// The model file under `tests/number_widths/`.
    pub file: &'static str,
    /// What the model is, for its header.
    pub about: String,
    /// The `const` items, as MSL text.
    pub constants: Vec<String>,
    /// The input carriers: name, MSL type, initial value as MSL text.
    pub inputs: Vec<(String, String, String)>,
    /// The table.
    pub cases: Vec<WidthCase>,
}

impl WidthBench {
    /// The directory of the models.
    const RUN: SourceRun = SourceRun::in_directory("tests/number_widths");

    /// The text of the model: the oscillator, the consts, one carrier per input and per case, and one reaction.
    pub fn model_text(&self) -> String {
        let mut text = format!("//! {}\n//! Generated from the table of `tests/source_number_widths.rs`: that test fails when this text differs.\n\n", self.about);
        text.push_str("pub module Osc(out clk: Bit = false) { f: f64 = 200M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\n\n");
        for constant in &self.constants {
            text.push_str(constant);
            text.push('\n');
        }
        text.push_str("\nmodule Bench() {\n    signal clk: Bit;\n");
        for (name, carrier_type, init) in &self.inputs {
            text.push_str(&format!("    signal {name}: Bit<{carrier_type}> = {init};\n"));
        }
        for case in &self.cases {
            text.push_str(&format!("    signal {}: Bit<{}> = {};\n", case.name, case.carrier_type, case.zero));
        }
        text.push_str("    let osc = Osc(clk) { f: 200M };\n}\ndigital for Bench {\n    on clk.rise() {\n");
        for case in &self.cases {
            text.push_str(&format!("        {} <- {};\n", case.name, case.expression));
        }
        text.push_str("    }\n}\n");
        text
    }

    /// The committed model is the table's text; with `WIDTH_MODELS_WRITE` set the file is rewritten instead.
    pub fn assert_model_current(&self) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/number_widths").join(self.file);
        let wanted = self.model_text();
        if std::env::var_os("WIDTH_MODELS_WRITE").is_some() {
            std::fs::write(&path, &wanted).expect("the model file is written");
        }
        let held = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(held == wanted, "{} is not the text of its table: run the test with WIDTH_MODELS_WRITE=1 to rewrite it", self.file);
    }

    /// The model `file` of the directory as a project.
    pub fn project(file: &str) -> crate::project::Project {
        Self::RUN.project(file)
    }

    /// The bytes the carriers `names` of `Bench` in `file` hold after the first rising edge.
    pub fn first_commits(file: &str, names: &[&str]) -> Vec<Vec<u8>> {
        Self::RUN.first_commits(file, names)
    }

    /// The message of the fault the transient of `top` of `file` runs into.
    pub fn fault(file: &str, top: &str) -> String {
        Self::RUN.the_fault(file, top)
    }

    /// Runs the model to its first edge and compares every case's committed bytes with its expectation;
    /// the failure names the case and its expression.
    pub fn assert_first_edge(&self) {
        self.assert_model_current();
        let names: Vec<&str> = self.cases.iter().map(|case| case.name.as_str()).collect();
        let committed = Self::RUN.first_commits(self.file, &names);
        for (case, got) in self.cases.iter().zip(committed) {
            assert_eq!(got, case.expected, "{}: {} <- {}", self.file, case.name, case.expression);
        }
    }
}

/// The bytes a carrier holds, by its type's layout.
pub struct ValueBytes;

impl ValueBytes {
    /// The low `bytes` bytes of `pattern`, little-endian.
    pub fn of_pattern(pattern: u128, bytes: usize) -> Vec<u8> {
        pattern.to_le_bytes()[..bytes].to_vec()
    }

    /// A `bool` carrier: one byte.
    pub fn of_bool(value: bool) -> Vec<u8> {
        vec![u8::from(value)]
    }

    /// The two's complement pattern of `limbs` as bytes, low limb first.
    pub fn of_limbs(limbs: &[u64]) -> Vec<u8> {
        limbs.iter().flat_map(|limb| limb.to_le_bytes()).collect()
    }
}
