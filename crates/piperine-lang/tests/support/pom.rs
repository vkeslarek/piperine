//! Helpers shared by the `pom_*` targets (`#[path]`-included, never a target
//! itself): build a design from one source text, list its diagnostic codes.

use piperine_lang::pom::Design;

/// A design of one in-memory root file named `top.msl`.
pub fn build(src: &str) -> Design {
    Design::builder().source("top.msl", src).build()
}

/// The code of every diagnostic of the design, in order.
pub fn codes(design: &Design) -> Vec<String> {
    design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
}
