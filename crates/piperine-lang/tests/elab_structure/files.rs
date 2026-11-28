//! Scope: files read at elaboration (`piperine_lang::elab::IncludedFiles`, PI-08 AC 1, 3 to 5,
//! spec part VI §20): `include_str!` in a `const` gives the file's text, the path resolved beside
//! the source file, each resolved path read once; an unreadable file is `E0612` naming the
//! resolved path, a file that is not UTF-8 is `E0613` with the byte offset of the first invalid
//! sequence; a file that changes later changes nothing until the design is elaborated again.


use std::path::{Path, PathBuf};

use piperine_ir::{Constant, Real};
use piperine_lang::elab::{CompositeKind, ConstEval, ConstFault, ConstTree, Elaborator, FileFault};
use piperine_lang::pom::{DeclId, DeclKind, Design, Reflect};
use piperine_lang::types::Types;
use piperine_lang_fixtures::elaboration::SourceElaboration;

/// The package of fixtures (`data.txt` (two lines), `latin.bin` (invalid UTF-8) and one root file
/// per case) and the source text of the cases that need a root file of their own.
struct Fixture;

impl Fixture {
    /// The path of the fixture `name`.
    fn path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/files").join(name)
    }

    /// The design of the root file at `path`, read from disk; it must hold no diagnostic of its own.
    fn design(path: &Path) -> Design {
        let design = Design::builder().root_file(path).build();
        assert!(design.diagnostics().is_empty(), "{:?}", SourceElaboration::codes(design.diagnostics()));
        design
    }

    /// The `const` item named `name`.
    fn constant(design: &Design, name: &str) -> DeclId {
        design.decls().into_iter().find(|decl| decl.name() == Some(name) && matches!(decl.kind(), DeclKind::Const(_))).unwrap_or_else(|| panic!("no const `{name}`")).id()
    }

    /// A top whose child reads the `const` `name`, `include_str!(file)`, in a reaction: the only
    /// place the subset takes a `str` (the `msg` of a `sim` record), so the `const` is reached.
    fn reading(name: &str, file: &str) -> String {
        format!(
            "const {name}: str = include_str!(\"{file}\");\nmodule Cell(in clk: Bit) {{ }}\ndigital for Cell {{ on clk.rise() {{ emit!(\"sim\", msg = {name}); }} }}\nmodule Top() {{ signal clk: Bit = false; let c = Cell(clk); }}\n"
        )
    }
}

#[test]
fn include_str_reads_the_file_beside_the_source() {
    // PI-08 AC 1, §20 rule 2: `const T: str = include_str!("data.txt");` is the text of the file
    // that sits beside the source file holding it, whatever the working directory is.
    let design = Fixture::design(&Fixture::path("reads.msl"));
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let mut eval = ConstEval::new(&design, &types);
    let text = eval.text(Fixture::constant(&design, "T")).expect("the file is readable");
    assert_eq!(&*text, "first line\nsecond line\n");
    let ir = Elaborator::new(&design).run("Top");
    assert!(ir.is_ok(), "elaboration reads the file without a diagnostic: {:?}", ir.err().map(|d| SourceElaboration::codes(&d)));
}

#[test]
fn an_unreadable_file_is_e0612_naming_the_path() {
    // PI-08 AC 3, §20 `E0612`: the report names the resolved path, beside the source file.
    let path = Fixture::path("unreadable.msl");
    let design = Fixture::design(&path);
    let err = Elaborator::new(&design).run("Top").expect_err("the file is absent");
    assert_eq!(SourceElaboration::codes(&err), ["E0612"]);
    let resolved = Fixture::path("missing.txt");
    assert!(err[0].diagnostic.message.contains(&resolved.display().to_string()), "{}", err[0].diagnostic.message);
    // D14: a source with no directory (an in-memory one) resolves a relative path against nothing.
    let in_memory = SourceElaboration::run(&Fixture::reading("T", "data.txt"), "Top").expect_err("no directory");
    assert_eq!(SourceElaboration::codes(&in_memory), ["E0612"]);
    assert!(in_memory[0].diagnostic.message.contains("no directory"), "{}", in_memory[0].diagnostic.message);
}

#[test]
fn a_file_that_is_not_utf8_is_e0613_with_the_offset() {
    // PI-08 AC 4, §20 rule 4: `latin.bin` is `valid ` (6 bytes) then `0xff`; the offset is the
    // first invalid byte, D15.
    let path = Fixture::path("not_utf8.msl");
    let design = Fixture::design(&path);
    let err = Elaborator::new(&design).run("Top").expect_err("the file is not UTF-8");
    assert_eq!(SourceElaboration::codes(&err), ["E0613"]);
    let message = &err[0].diagnostic.message;
    assert!(message.contains(&Fixture::path("latin.bin").display().to_string()) && message.contains("at byte 6"), "{message}");
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let mut eval = ConstEval::new(&design, &types);
    match eval.text(Fixture::constant(&design, "L")) {
        Err(ConstFault::File { fault: FileFault::NotUtf8 { offset, .. }, .. }) => assert_eq!(offset, 6),
        other => panic!("expected the offset of the first invalid sequence, got {other:?}"),
    }
}

/// A directory of its own under the temporary one, removed when it drops.
struct Scratch {
    directory: PathBuf,
}

impl Scratch {
    /// A new empty directory named for `case`.
    fn new(case: &str) -> Scratch {
        let directory = std::env::temp_dir().join(format!("piperine-elab-files-{case}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a scratch directory");
        Scratch { directory }
    }

    /// Write `text` to `name` in the directory and give its path.
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.directory.join(name);
        std::fs::write(&path, text).expect("a scratch file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn a_changed_file_needs_a_new_elaboration() {
    // PI-08 AC 5, §20 rule 3: the text is read once; a change to the file reaches nothing until a
    // new elaboration reads it again.
    let scratch = Scratch::new("changed");
    scratch.write("data.txt", "before");
    let root = scratch.write("top.msl", &Fixture::reading("T", "data.txt"));
    let design = Fixture::design(&root);
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let mut eval = ConstEval::new(&design, &types);
    let decl = Fixture::constant(&design, "T");
    assert_eq!(&*eval.text(decl).expect("readable"), "before");
    scratch.write("data.txt", "after");
    assert_eq!(&*eval.text(decl).expect("readable"), "before", "the same elaboration keeps the text it read");
    let mut again = ConstEval::new(&design, &types);
    assert_eq!(&*again.text(decl).expect("readable"), "after", "a new elaboration reads the file again");
}

#[test]
fn one_file_named_twice_is_read_once() {
    // §20 rule 3: two `const`s naming `data.txt` open it once.
    let design = Fixture::design(&Fixture::path("reads.msl"));
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let mut eval = ConstEval::new(&design, &types);
    let (first, second) = (eval.text(Fixture::constant(&design, "T")).expect("readable"), eval.text(Fixture::constant(&design, "U")).expect("readable"));
    assert_eq!(first, second);
    assert_eq!(eval.files_read(), 1, "one resolved path, one read");
}

/// The sources the file and table tests elaborate, and the readings of what they give: the owner of the helpers they share.
struct Parsed;

impl Parsed {
    /// A source whose `const` `name` of type `ty` is `parser(include_str!(file))`, read in `analog` so the checker reaches it.
    pub fn source(name: &str, ty: &str, parser: &str, file: &str) -> String {
        format!("const {name}: {ty} = {parser}(include_str!(\"{file}\"));\nmodule Top() {{ }}\nanalog for Top {{ let x = {name}[0]; }}\n")
    }


    /// The folded tree of `const` `name` of the package at `root`, or the diagnostics of elaborating it.
    pub fn folded(root: &Path, name: &str) -> Result<ConstTree, Vec<String>> {
        let design = Fixture::design(root);
        let types = Types::check(&design, "Top").map_err(|found| found.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>())?;
        ConstEval::new(&design, &types).constant_tree(Fixture::constant(&design, name)).map_err(|fault| vec![format!("{fault:?}")])
    }


    /// The diagnostics (code and message) elaborating the package at `root` gives.
    pub fn refused(scratch: &Scratch, source: &str) -> Vec<(String, String)> {
        let root = scratch.write("top.msl", source);
        let design = Fixture::design(&root);
        Elaborator::new(&design).run("Top").expect_err("the text is refused").iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }


    /// A source whose `analog` block contributes `table_model(v(a), T, control)`, `T` being `samples` written out.
    pub fn table_model_source(samples: &str, count: usize, control: &str) -> String {
        format!("const T: [(f64, f64); {count}] = [{samples}];\nmodule Top() {{ signal a: Electrical; }}\nanalog for Top {{ i(a) +> table_model(v(a), T, \"{control}\"); }}\n")
    }


    /// The codes and messages of elaborating `source` with `Top` as the top.
    pub fn refusals(source: &str) -> Vec<(String, String)> {
        SourceElaboration::run(source, "Top").expect_err("the table model is refused").iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }


}

/// The trees of the values the tests expect.
struct Trees;

impl Trees {
    /// The tree of the real `value`.
    pub fn real(value: f64) -> ConstTree {
        ConstTree::Scalar(Constant::Real(Real::new(value)))
    }


    /// The tree of the integer `value`.
    pub fn int(value: i128) -> ConstTree {
        ConstTree::Scalar(Constant::Int(value))
    }

}

#[test]
fn parse_table_reads_64_rows_in_order() {
    // PI-09 AC 1, §22.6.4 rule 3 and the resolutions: one row per line, whitespace columns, blank lines skipped, `#` comments, SI suffixes.
    // Row k is `k * 0.1` then `k` mA (`k m`): the 64 pairs come in the file's order.
    let scratch = Scratch::new("table64");
    let mut text = String::from("# the measured I-V of the diode\n\n");
    for k in 0..64 {
        text.push_str(&format!("{} {k}m  # row {k}\n", f64::from(k) * 0.1));
        if k == 10 {
            text.push_str("\n");
        }
    }
    scratch.write("diode_iv.tbl", &text);
    let root = scratch.write("top.msl", &Parsed::source("IV", "[(f64, f64); 64]", "parse_table", "diode_iv.tbl"));
    let ConstTree::Composite { kind: CompositeKind::Array, parts } = Parsed::folded(&root, "IV").expect("the table folds") else { panic!("an array") };
    assert_eq!(parts.len(), 64);
    for (k, row) in parts.iter().enumerate() {
        let ConstTree::Composite { kind: CompositeKind::Tuple, parts: pair } = row else { panic!("a pair") };
        assert_eq!(pair, &[Trees::real(k as f64 * 0.1), Trees::real(k as f64 * 1e-3)], "row {k}");
    }
    // a table of one number per row is an array of `f64`
    scratch.write("one.tbl", "1k\n2.5\n");
    let root = scratch.write("one.msl", &Parsed::source("T", "[f64; 2]", "parse_table", "one.tbl").replace("[0]", "[1]"));
    assert_eq!(Parsed::folded(&root, "T").expect("folds"), ConstTree::Composite { kind: CompositeKind::Array, parts: vec![Trees::real(1000.0), Trees::real(2.5)] });
}

#[test]
fn a_table_of_the_wrong_shape_is_e0614_naming_file_and_row() {
    // PI-09 AC 3, §22.6.4 rule 5, §20: a row with another number of columns than the type's, or another number of rows, is `E0614`
    // naming the file and the row.
    let scratch = Scratch::new("shape");
    scratch.write("wide.tbl", "1 2\n3 4 5\n");
    let found = Parsed::refused(&scratch, &Parsed::source("T", "[(f64, f64); 2]", "parse_table", "wide.tbl"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, "E0614");
    assert!(found[0].1.contains("wide.tbl") && found[0].1.contains("row 2"), "names the file and the row: {}", found[0].1);
    scratch.write("short.tbl", "1 2\n");
    let found = Parsed::refused(&scratch, &Parsed::source("T", "[(f64, f64); 2]", "parse_table", "short.tbl"));
    assert_eq!(found[0].0, "E0614", "{found:?}");
    assert!(found[0].1.contains("short.tbl") && found[0].1.contains("1 rows"), "{}", found[0].1);
}

#[test]
fn a_token_that_is_not_a_real_is_e0706() {
    // PI-09 AC 4, §22.6.4 rule 6: `abc` on the third line is no real literal: `E0706` naming the file, the row and the token; so are `inf` and an exponent with an SI suffix (`1e-3k`, Appendix A.2 rule 4).
    let scratch = Scratch::new("token");
    for (name, text, token, row) in [("bad.tbl", "1 2\n\n3 abc\n", "abc", "row 3"), ("inf.tbl", "1 inf\n", "inf", "row 1"), ("exp.tbl", "1 1e-3k\n", "1e-3k", "row 1")] {
        scratch.write(name, text);
        let found = Parsed::refused(&scratch, &Parsed::source("T", "[(f64, f64); 2]", "parse_table", name));
        assert_eq!(found[0].0, "E0706", "{name}: {found:?}");
        assert!(found[0].1.contains(name) && found[0].1.contains(row) && found[0].1.contains(&format!("`{token}`")), "{}", found[0].1);
    }
}

#[test]
fn an_infinite_row_value_is_e0706() {
    // PI-09 edge case: `1e999` is a real literal whose nearest `f64` is infinite, `E0706` naming the row.
    let scratch = Scratch::new("infinite");
    scratch.write("big.tbl", "1 2\n3 1e999\n");
    let found = Parsed::refused(&scratch, &Parsed::source("T", "[(f64, f64); 2]", "parse_table", "big.tbl"));
    assert_eq!(found[0].0, "E0706", "{found:?}");
    assert!(found[0].1.contains("row 2") && found[0].1.contains("1e999") && found[0].1.contains("infinite"), "{}", found[0].1);
}

#[test]
fn parse_hex_reads_words_at_their_addresses() {
    // PI-09 AC 2, §22.6.4 resolution: the `$readmemh` format: words in order from address 0, `_` inside a word, `//` and
    // `/* */` comments, and `@address` records that move where the next word goes.
    let scratch = Scratch::new("hex");
    // `@0` then 8 words, comments of both forms and a `_` inside a word
    scratch.write("full.hex", "// boot image\n@0 DE_AD /* between words */ BEEF 7 9 00ff 01 /* two\nlines */ 02 03\n");
    let root_full = scratch.write("full.msl", &Parsed::source("ROM", "[u32; 8]", "parse_hex", "full.hex"));
    let ConstTree::Composite { kind: CompositeKind::Array, parts } = Parsed::folded(&root_full, "ROM").expect("the image folds") else { panic!("an array") };
    assert_eq!(parts, [Trees::int(0xDEAD), Trees::int(0xBEEF), Trees::int(7), Trees::int(9), Trees::int(0xFF), Trees::int(1), Trees::int(2), Trees::int(3)]);
    // the addresses move the next word: `@4` then `@2`
    scratch.write("moved.hex", "@4 0a 0b\n@0 01 02\n@2 03 04\n");
    let root_moved = scratch.write("moved.msl", &Parsed::source("ROM", "[u8; 6]", "parse_hex", "moved.hex"));
    let ConstTree::Composite { parts, .. } = Parsed::folded(&root_moved, "ROM").expect("folds") else { panic!("an array") };
    assert_eq!(parts, [Trees::int(1), Trees::int(2), Trees::int(3), Trees::int(4), Trees::int(0xa), Trees::int(0xb)]);
}

#[test]
fn x_and_z_digits_are_e0706_integers_hold_no_unknown() {
    // PI-09 AC 2 and the §22.6.4 resolution (explicit-unknowns, MD-78: no integer holds an unknown level): a digit `x` or `z` is
    // `E0706` naming the file, the row and the word.
    let scratch = Scratch::new("hexunknown");
    for word in ["0x", "zz", "1X"] {
        scratch.write("u.hex", &format!("00\n{word}\n"));
        let found = Parsed::refused(&scratch, &Parsed::source("ROM", "[u8; 2]", "parse_hex", "u.hex"));
        assert_eq!(found[0].0, "E0706", "{word}: {found:?}");
        assert!(found[0].1.contains("u.hex") && found[0].1.contains("row 2") && found[0].1.contains(&format!("`{word}`")) && found[0].1.contains("unknown"), "{}", found[0].1);
    }
}

#[test]
fn an_address_the_text_does_not_write_is_e0614() {
    // PI-09 AC 3 (§22.6.4 resolution): an index the text never writes, and an address past the array, are `E0614`.
    let scratch = Scratch::new("hexaddress");
    scratch.write("gap.hex", "01 02 @3 04\n");
    let found = Parsed::refused(&scratch, &Parsed::source("ROM", "[u8; 4]", "parse_hex", "gap.hex"));
    assert_eq!(found[0].0, "E0614", "{found:?}");
    assert!(found[0].1.contains("gap.hex") && found[0].1.contains("address 2"), "{}", found[0].1);
    scratch.write("past.hex", "01 02 03\n");
    let found = Parsed::refused(&scratch, &Parsed::source("ROM", "[u8; 2]", "parse_hex", "past.hex"));
    assert_eq!(found[0].0, "E0614", "{found:?}");
    assert!(found[0].1.contains("past.hex") && found[0].1.contains("address 2") && found[0].1.contains("row 1"), "{}", found[0].1);
}

#[test]
fn a_word_that_is_not_hex_is_e0706() {
    // PI-09 AC 4: `g1` is no hexadecimal word, and `1ff` does not fit `u8`: each is `E0706` naming file, row and word.
    let scratch = Scratch::new("hexword");
    for (word, why) in [("g1", "not a hexadecimal word"), ("1ff", "does not fit 8 bits")] {
        scratch.write("w.hex", &format!("00\n{word}\n"));
        let found = Parsed::refused(&scratch, &Parsed::source("ROM", "[u8; 2]", "parse_hex", "w.hex"));
        assert_eq!(found[0].0, "E0706", "{word}: {found:?}");
        assert!(found[0].1.contains("w.hex") && found[0].1.contains("row 2") && found[0].1.contains(&format!("`{word}`")) && found[0].1.contains(why), "{}", found[0].1);
    }
}

#[test]
fn a_bad_control_string_is_e0703() {
    // PI-10 AC 6, §22.2.15 rule 4: a control string that is not one interpolation character and up to two extrapolation characters is
    // `E0703`; the report names the string.
    for control in ["9", "", "1X", "1CLE", "C1"] {
        let found = Parsed::refusals(&Parsed::table_model_source("(0.0, 0.0), (1.0, 1.0)", 2, control));
        assert_eq!(found.len(), 1, "{control}: {found:?}");
        assert_eq!(found[0].0, "E0703", "{control}: {found:?}");
        assert!(found[0].1.contains(&format!("`{control}`")), "{}", found[0].1);
    }
    // every form of the table in §22.2.15 is accepted
    for control in ["D", "1", "2C", "3", "1LE", "3EC", "DCC"] {
        let samples = "(0.0, 0.0), (1.0, 1.0), (2.0, 4.0), (3.0, 9.0)";
        SourceElaboration::run(&Parsed::table_model_source(samples, 4, control), "Top").unwrap_or_else(|d| panic!("`{control}`: {:?}", SourceElaboration::codes(&d)));
    }
}

#[test]
fn non_increasing_abscissae_are_e0704() {
    // PI-10 AC 7, §22.2.15 rule 1: abscissae that do not strictly increase are `E0704` naming the first offending row (from 1).
    let found = Parsed::refusals(&Parsed::table_model_source("(0.0, 0.0), (1.0, 1.0), (1.0, 2.0), (0.5, 3.0)", 4, "1"));
    assert_eq!(found[0].0, "E0704", "{found:?}");
    assert!(found[0].1.contains("row 3"), "the first offending row is the third: {}", found[0].1);
}

#[test]
fn too_few_points_for_the_degree_is_e0704() {
    // PI-10 AC 7: a quadratic spline needs three points and a cubic one four.
    for (control, samples, count, needed) in [("2", "(0.0, 0.0), (1.0, 1.0)", 2, 3), ("3", "(0.0, 0.0), (1.0, 1.0), (2.0, 4.0)", 3, 4)] {
        let found = Parsed::refusals(&Parsed::table_model_source(samples, count, control));
        assert_eq!(found[0].0, "E0704", "{control}: {found:?}");
        assert!(found[0].1.contains(&format!("at least {needed}")), "{}", found[0].1);
    }
}

#[test]
fn a_one_row_table_under_d_is_e0704() {
    // PI-10 edge case: one sample is no table under `D` either.
    let found = Parsed::refusals(&Parsed::table_model_source("(0.0, 1.0)", 1, "D"));
    assert_eq!(found[0].0, "E0704", "{found:?}");
    assert!(found[0].1.contains("1 point"), "{}", found[0].1);
}
