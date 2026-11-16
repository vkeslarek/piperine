//! The printer: the IR and its plans as one Rust source text, with fixed layout rules (4-space
//! indentation, one statement per line, every item under a comment naming its IR path and
//! origin) so the text reads the same on every build (RS-02).

pub mod analog;
pub mod cross;
pub mod digital;
pub mod library;
pub mod limit;
pub mod reaction;
pub mod shape;
pub mod site;
pub mod text;
pub mod value;

pub use analog::AnalogPrinter;
pub use digital::DigitalPrinter;
pub use library::LibraryPrinter;
pub use shape::ShapePrinter;
pub use text::Text;
pub use value::ValuePrinter;
