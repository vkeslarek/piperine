//! The printer: the IR and its plans as one Rust source text, with fixed layout rules (4-space
//! indentation, one statement per line, every item under a comment naming its IR path and
//! origin) so the text reads the same on every build (RS-02).

pub mod analog;
pub mod contribution;
pub mod cross;
pub mod derive;
pub mod delay;
pub mod derivative;
pub mod digital;
pub mod expression;
pub mod library;
pub mod lift;
pub mod limit;
pub mod noise;
pub mod reaction;
pub mod record;
pub mod shape;
pub mod site;
pub mod statement;
pub mod text;
pub mod trace;
pub mod types;

pub use analog::AnalogPrinter;
pub use contribution::ContributionSums;
pub use delay::DelayPrinter;
pub use derivative::DerivativePrinter;
pub use derive::DerivePrinter;
pub use digital::DigitalPrinter;
pub use expression::ExpressionPrinter;
pub use library::LibraryPrinter;
pub use noise::NoisePrinter;
pub use record::RecordPrinter;
pub use lift::RealLift;
pub use shape::ShapePrinter;
pub use statement::StatementPrinter;
pub use text::Text;
pub use trace::TracePrinter;
pub use types::TypeItems;
