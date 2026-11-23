//! The runtime source: the few types generated code needs and Rust lacks. Each file is a module
//! of this crate, so its unit tests run under `cargo test` with no `rustc` call, and is embedded
//! as text to be written into every cache entry as `rt.rs` (and, for a design with digital
//! forms, the files beside it).

pub mod array;
pub mod commits;
pub mod control;
pub mod pin;
pub mod int;
pub mod real;
pub mod numbers;
pub mod table;
pub mod widths;

/// The text of `rt/real.rs`, as generated code compiles it.
pub const REAL: &str = include_str!("rt/real.rs");
/// The text of `rt/int.rs`, as generated code compiles it.
pub const INT: &str = include_str!("rt/int.rs");
/// The text of `rt/array.rs`, as generated code compiles it.
pub const ARRAY: &str = include_str!("rt/array.rs");
/// The text of `rt/commits.rs`, as generated code compiles it.
pub const COMMITS: &str = include_str!("rt/commits.rs");
/// The text of `rt/control.rs`, as generated code compiles it.
pub const CONTROL: &str = include_str!("rt/control.rs");
/// The text of `rt/pin.rs`, as generated code compiles it.
pub const PIN: &str = include_str!("rt/pin.rs");
/// The text of `rt/table.rs`, as generated code compiles it.
pub const TABLE: &str = include_str!("rt/table.rs");

/// The files of `piperine-numbers`, as `rt/numbers/<file>.rs`: embedded verbatim, so a design's runtime holds the
/// one implementation of every operation that elaboration, the host and the generated code share.
pub const NUMBER_FILES: [(&str, &str); 15] = [
    ("rt/numbers/arithmetic.rs", include_str!("../../piperine-numbers/src/arithmetic.rs")),
    ("rt/numbers/constants.rs", include_str!("../../piperine-numbers/src/constants.rs")),
    ("rt/numbers/convert.rs", include_str!("../../piperine-numbers/src/convert.rs")),
    ("rt/numbers/decimal.rs", include_str!("../../piperine-numbers/src/decimal.rs")),
    ("rt/numbers/elementary.rs", include_str!("../../piperine-numbers/src/elementary.rs")),
    ("rt/numbers/format.rs", include_str!("../../piperine-numbers/src/format.rs")),
    ("rt/numbers/half.rs", include_str!("../../piperine-numbers/src/half.rs")),
    ("rt/numbers/limbs.rs", include_str!("../../piperine-numbers/src/limbs.rs")),
    ("rt/numbers/power.rs", include_str!("../../piperine-numbers/src/power.rs")),
    ("rt/numbers/precision.rs", include_str!("../../piperine-numbers/src/precision.rs")),
    ("rt/numbers/quad.rs", include_str!("../../piperine-numbers/src/quad.rs")),
    ("rt/numbers/reduction.rs", include_str!("../../piperine-numbers/src/reduction.rs")),
    ("rt/numbers/transcendental.rs", include_str!("../../piperine-numbers/src/transcendental.rs")),
    ("rt/numbers/wide.rs", include_str!("../../piperine-numbers/src/wide.rs")),
    ("rt/numbers/ziv.rs", include_str!("../../piperine-numbers/src/ziv.rs")),
];
/// The text of `rt/widths.rs`, as generated code compiles it.
pub const WIDTHS: &str = include_str!("rt/widths.rs");

/// The `rt.rs` of a design with digital forms: the files as its modules, their names
/// re-exported, so generated code spells `rt::Real`, `rt::UInt`, `rt::Carried` whatever file
/// each is in.
const ROOT: &str = "\
//! The runtime of a design with digital forms: reals and their derivatives (real.rs), integers
//! that wrap at their width (int.rs), the types a pin carries (pin.rs), raising a panic and the
//! bounds of a model's own values (control.rs), the commits of a reaction (commits.rs) and the
//! interpolant of a `table_model` (table.rs).

pub mod array;
pub mod commits;
pub mod control;
pub mod pin;
pub mod int;
pub mod real;
pub mod table;

pub use array::{CarrierArray, ElementIndex};
pub use commits::Commits;
pub use control::{Depth, Index, Panic, Trips};
pub use pin::{Carried, LevelWords};
pub use int::{Int, UInt, Unwrap};
pub use real::{Dual, Real};
pub use table::TableModel;
";

/// The lines the root `rt.rs` gains when the design uses a number width.
const NUMBERS_FORMS: &str = "\
pub mod numbers;
pub mod widths;

pub use numbers::half::Float16;
pub use numbers::quad::Float128;
pub use numbers::wide::{WideInt, WideUInt};
pub use widths::{Convert, FloatRepr, IntegerRepr};
";

/// The generated `rt/numbers.rs`: the library's files as its modules.
const NUMBERS_ROOT: &str = "\
//! The numeric library (`piperine-numbers`), embedded verbatim: integers of any width, `f16` and `f128`.

pub mod arithmetic;
pub mod constants;
pub mod convert;
pub mod decimal;
pub mod elementary;
pub mod format;
pub mod half;
pub mod limbs;
pub mod power;
pub mod precision;
pub mod quad;
pub mod reduction;
pub mod transcendental;
pub mod wide;
pub mod ziv;
";

/// The forms of a design that decide which runtime files it gets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeForms {
    /// A digital form, a loop, a carried param, a table or a derive: the files of `rt` and not `real.rs` alone.
    pub digital: bool,
    /// A number width (`f32` excepted: it is Rust's own): `rt/numbers/*.rs` and `rt/widths.rs` are added.
    pub numbers: bool,
}

/// The files a design's cache entry holds as its runtime.
#[derive(Debug)]
pub struct Runtime;

impl Runtime {
    /// The runtime files of a design, `(path under the entry, text)`, the root `rt.rs` first. A
    /// design with no digital form gets `real.rs` alone as `rt.rs`, so its entry and its cache
    /// key are what an analog-only build has always had.
    pub fn files(digital: bool) -> Vec<(&'static str, &'static str)> {
        Self::files_for(RuntimeForms { digital, numbers: false })
    }

    /// The root `rt.rs` of a design with a number width: the digital root and the lines of `NUMBERS_FORMS`.
    fn root_with_numbers() -> &'static str {
        static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        TEXT.get_or_init(|| format!("{ROOT}\n{NUMBERS_FORMS}"))
    }

    /// The runtime files of a design with the forms `forms`. A design with a number width gets the
    /// files of a digital design plus `rt/numbers.rs`, `rt/numbers/*.rs` and `rt/widths.rs`, and a root that
    /// re-exports them; every other design gets exactly the files it had before.
    pub fn files_for(forms: RuntimeForms) -> Vec<(&'static str, &'static str)> {
        if forms.numbers {
            let mut files = vec![("rt.rs", Self::root_with_numbers())];
            files.extend(Self::files(true).into_iter().skip(1));
            files.push(("rt/numbers.rs", NUMBERS_ROOT));
            files.extend(NUMBER_FILES);
            files.push(("rt/widths.rs", WIDTHS));
            files
        } else if forms.digital {
            vec![("rt.rs", ROOT), ("rt/real.rs", REAL), ("rt/int.rs", INT), ("rt/pin.rs", PIN), ("rt/control.rs", CONTROL), ("rt/commits.rs", COMMITS), ("rt/array.rs", ARRAY), ("rt/table.rs", TABLE)]
        } else {
            vec![("rt.rs", REAL)]
        }
    }
}
