//! The number widths in the transpiler (composite-values phase D, CV-29 to CV-32; MD-63 item 1):
//! the runtime a design gets (`Runtime::files_for`), and the generated code of `f32`, `f16`,
//! `f128` and integers wider than 64 bits compiled by `rustc` and run. The oracle for a result is
//! Rust's own `f32` arithmetic evaluated in the test and `piperine-numbers` called in the test.

use piperine_transpile::rt::{NUMBER_FILES, Runtime, RuntimeForms};

/// The helpers of this target.
struct Files;

impl Files {
    /// The paths of runtime files.
    fn paths(files: Vec<(&'static str, &'static str)>) -> Vec<&'static str> {
        files.into_iter().map(|(path, _)| path).collect()
    }
}

#[test]
fn a_design_without_number_widths_gets_todays_runtime_files() {
    // MD-63 item 1 and the cache keys: a design that uses none of the new types gets exactly the files it had,
    // whatever the switch `numbers: false` says, so no existing cache entry or golden moves
    assert_eq!(Files::paths(Runtime::files(false)), ["rt.rs"], "an analog-only design has `real.rs` alone as `rt.rs`");
    assert_eq!(
        Files::paths(Runtime::files(true)),
        ["rt.rs", "rt/real.rs", "rt/int.rs", "rt/pin.rs", "rt/control.rs", "rt/commits.rs", "rt/array.rs", "rt/table.rs"],
        "a digital design gets the files of `rt`"
    );
    for digital in [false, true] {
        assert_eq!(Runtime::files_for(RuntimeForms { digital, numbers: false }), Runtime::files(digital), "digital: {digital}");
    }

    // a design that uses a number width gets those files, then `rt/numbers.rs`, every library file and `rt/widths.rs`,
    // and a root that re-exports the types the printer spells
    let with = Runtime::files_for(RuntimeForms { digital: false, numbers: true });
    let with_paths = Files::paths(with.clone());
    assert_eq!(with_paths[1..8], Files::paths(Runtime::files(true))[1..], "the digital files follow the root unchanged");
    assert!(with_paths.contains(&"rt/numbers.rs") && with_paths.contains(&"rt/widths.rs"));
    for (path, text) in NUMBER_FILES {
        assert!(with.iter().any(|(file, body)| *file == path && *body == text), "{path} is embedded verbatim");
    }
    let root = with[0].1;
    for spelled in ["pub use numbers::half::Float16;", "pub use numbers::quad::Float128;", "pub use numbers::wide::{WideInt, WideUInt};", "pub use widths::{Convert, FloatRepr, IntegerRepr};"] {
        assert!(root.contains(spelled), "the root re-exports `{spelled}`:\n{root}");
    }
    // the library's own files are what the leaf crate holds on disk: verbatim, not a copy that can drift
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../piperine-numbers/src");
    for (path, text) in NUMBER_FILES {
        let name = path.trim_start_matches("rt/numbers/");
        assert_eq!(std::fs::read_to_string(directory.join(name)).expect("a library file"), text, "{name}");
    }
}
