//! `render`: the text a user reads. Each renderer lays out one kind of value and writes it to
//! the stream it is given; none decides what is shown (that is `piperine-api`'s).
//!
//! - [`number`]: the one place a cell becomes text.
//! - [`table`]: a result table as aligned text.

pub mod number;
pub mod table;
