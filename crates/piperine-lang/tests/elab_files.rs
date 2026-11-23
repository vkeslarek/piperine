//! Scope: files read at elaboration (`piperine_lang::elab::IncludedFiles`, PI-08 AC 1, 3 to 5,
//! spec part VI §20): `include_str!` in a `const` gives the file's text, the path resolved beside
//! the source file, each resolved path read once; an unreadable file is `E0612` naming the
//! resolved path, a file that is not UTF-8 is `E0613` with the byte offset of the first invalid
//! sequence; a file that changes later changes nothing until the design is elaborated again.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use std::path::{Path, PathBuf};

use piperine_lang::elab::{ConstEval, ConstFault, Elaborator, FileFault};
use piperine_lang::pom::{DeclId, DeclKind, Design, Reflect};
use piperine_lang::types::Types;
use support::Elab;

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
        assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
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
    assert!(ir.is_ok(), "elaboration reads the file without a diagnostic: {:?}", ir.err().map(|d| Elab::codes(&d)));
}

#[test]
fn an_unreadable_file_is_e0612_naming_the_path() {
    // PI-08 AC 3, §20 `E0612`: the report names the resolved path, beside the source file.
    let path = Fixture::path("unreadable.msl");
    let design = Fixture::design(&path);
    let err = Elaborator::new(&design).run("Top").expect_err("the file is absent");
    assert_eq!(Elab::codes(&err), ["E0612"]);
    let resolved = Fixture::path("missing.txt");
    assert!(err[0].diagnostic.message.contains(&resolved.display().to_string()), "{}", err[0].diagnostic.message);
    // D14: a source with no directory (an in-memory one) resolves a relative path against nothing.
    let in_memory = Elab::run(&Fixture::reading("T", "data.txt"), "Top").expect_err("no directory");
    assert_eq!(Elab::codes(&in_memory), ["E0612"]);
    assert!(in_memory[0].diagnostic.message.contains("no directory"), "{}", in_memory[0].diagnostic.message);
}

#[test]
fn a_file_that_is_not_utf8_is_e0613_with_the_offset() {
    // PI-08 AC 4, §20 rule 4: `latin.bin` is `valid ` (6 bytes) then `0xff`; the offset is the
    // first invalid byte, D15.
    let path = Fixture::path("not_utf8.msl");
    let design = Fixture::design(&path);
    let err = Elaborator::new(&design).run("Top").expect_err("the file is not UTF-8");
    assert_eq!(Elab::codes(&err), ["E0613"]);
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
