//! The numeric library of `piperine-numbers`, compiled into this crate from the leaf crate's own
//! files so `rt/widths.rs` and its tests run under `cargo test` (the same files are embedded as
//! text and written into a design's cache entry as `rt/numbers/*.rs`, `Runtime::files_for`).

#[path = "../../../piperine-numbers/src/arithmetic.rs"]
pub mod arithmetic;
#[path = "../../../piperine-numbers/src/constants.rs"]
pub mod constants;
#[path = "../../../piperine-numbers/src/convert.rs"]
pub mod convert;
#[path = "../../../piperine-numbers/src/decimal.rs"]
pub mod decimal;
#[path = "../../../piperine-numbers/src/elementary.rs"]
pub mod elementary;
#[path = "../../../piperine-numbers/src/format.rs"]
pub mod format;
#[path = "../../../piperine-numbers/src/half.rs"]
pub mod half;
#[path = "../../../piperine-numbers/src/limbs.rs"]
pub mod limbs;
#[path = "../../../piperine-numbers/src/power.rs"]
pub mod power;
#[path = "../../../piperine-numbers/src/precision.rs"]
pub mod precision;
#[path = "../../../piperine-numbers/src/quad.rs"]
pub mod quad;
#[path = "../../../piperine-numbers/src/reduction.rs"]
pub mod reduction;
#[path = "../../../piperine-numbers/src/transcendental.rs"]
pub mod transcendental;
#[path = "../../../piperine-numbers/src/wide.rs"]
pub mod wide;
#[path = "../../../piperine-numbers/src/ziv.rs"]
pub mod ziv;
