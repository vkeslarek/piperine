//! The standard library, embedded in the binary as a source tree: the files the builder lowers
//! after every project file unless `DesignBuilder::std_source` replaces them.

/// The embedded files as `(path, text)`; the first is the root of `std`.
pub(crate) const EMBEDDED: [(&str, &str); 8] = [
    ("std/lib.msl", include_str!("../../std/lib.msl")),
    ("std/constants.msl", include_str!("../../std/constants.msl")),
    ("std/carriers.msl", include_str!("../../std/carriers.msl")),
    ("std/values.msl", include_str!("../../std/values.msl")),
    ("std/analog.msl", include_str!("../../std/analog.msl")),
    ("std/nets.msl", include_str!("../../std/nets.msl")),
    ("std/convert.msl", include_str!("../../std/convert.msl")),
    ("std/prelude.msl", include_str!("../../std/prelude.msl")),
];
