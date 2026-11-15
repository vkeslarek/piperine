//! The standard library, embedded in the binary as a source tree: the files the builder lowers
//! after every project file unless `DesignBuilder::std_source` replaces them.

/// The embedded files as `(path, text)`; the first is the root of `std`.
pub(crate) const EMBEDDED: [(&str, &str); 2] = [
    ("std/lib.msl", include_str!("../../std/lib.msl")),
    ("std/prelude.msl", include_str!("../../std/prelude.msl")),
];
