//! The floats of `piperine-numbers` against exact rational arithmetic: the conversions between every pair of widths, `BinaryFormat::round` and the decimal text, `Float16`, `Float128`, the multi-precision reference constants and the binary128 transcendentals against committed vectors. Each former target is a module of this one.

mod conversions;
mod exact;
mod exact_algebra;
mod exact_integers;
mod exact_nearest;
mod exact_parts;
mod exact_root;
mod exact_scaled;
mod exact_text;
mod exact_values;
mod formats;
mod half;
mod oracle;
mod precision;
mod quad;
mod transcendental;
