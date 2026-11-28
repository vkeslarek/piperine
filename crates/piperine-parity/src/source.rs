//! The readers and runners the `source_*` targets share: a model file run on the transpiled backend (`run`), the bytes of
//! a carrier (`bytes`), the fields of a record (`fields`), the unit-branch oracle (`network`), the place operators (`operators`) and the number-width cases (`widths`).

pub mod bytes;
pub mod fields;
pub mod network;
pub mod operators;
pub mod run;
pub mod widths;
