//! `render`: the text a user reads. Each renderer lays out one kind of value and writes it to
//! the stream it is given; none decides what is shown (that is `piperine-api`'s).
//!
//! - [`csv`]: a result table as CSV.
//! - [`json`]: an `emit!` record as one JSON object.
//! - [`svg`]: a plot as an SVG document.
//! - [`number`]: the one place a cell becomes text.
//! - [`table`]: a result table as aligned text.
//! - [`record`]: an `emit!` record as one line.
//! - [`diagnostic`]: a failure with its code, file, line and excerpt.

pub mod csv;
pub mod diagnostic;
pub mod json;
pub mod number;
pub mod record;
pub mod svg;
pub mod table;
