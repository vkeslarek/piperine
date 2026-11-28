//! The diagnostic codes of the lowering tests.

use crate::pom::PomSource;

/// The codes of the diagnostics that parsing and lowering produce, without the binding errors.
pub struct LoweringCodes;

impl LoweringCodes {
    /// The code of every diagnostic that parsing and lowering produce: all of
    /// them but the binding errors name resolution adds on top (POM-08..10:
    /// `E0161` for a name, `E0330`, `E0457`, `E0401`..`E0403`,
    /// `E0343`). A lowering test over a source that mentions names it does not
    /// declare asserts this list is empty.
    pub fn of(design: &piperine_lang::pom::Design) -> Vec<String> {
        const BINDING: [&str; 7] = ["E0161", "E0330", "E0457", "E0401", "E0402", "E0403", "E0343"];
        PomSource::codes(design).into_iter().filter(|c| !BINDING.contains(&c.as_str())).collect()
    }
}
