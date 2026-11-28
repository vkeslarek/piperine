//! Helpers shared by the `pom_*` targets (`#[path]`-included, never a target
//! itself): build a design from one source text, list its diagnostic codes.

use piperine_lang::pom::Design;

/// A design built from one source text and the codes of its diagnostics.
pub struct PomSource;

impl PomSource {
    /// A design of one in-memory root file named `top.msl`, over an empty standard library: the
    /// targets that use it test lowering, resolution and structure, not the library's content.
    pub fn build(src: &str) -> Design {
        Design::builder().std_source("lib.msl", "pub mod prelude;\n").std_source("prelude.msl", "").source("top.msl", src).build()
    }

    /// The code of every diagnostic of the design, in order.
    pub fn codes(design: &Design) -> Vec<String> {
        design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
    }
}

