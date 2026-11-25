//! Helpers shared by the `elab_*` targets (`#[path]`-included, never a target itself): build a
//! design over the real standard library, elaborate it, list diagnostic codes.

use piperine_ir::Ir;
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::{Design, DesignDiagnostic};

pub struct Elab;

impl Elab {
    /// A design of one in-memory root file `top.msl` over the real standard library.
    pub fn design(src: &str) -> Design {
        Design::builder().source("top.msl", src).build()
    }

    /// Elaborate `top` of `src`; the design must hold no diagnostic of its own.
    pub fn run(src: &str, top: &str) -> Result<Ir, Vec<DesignDiagnostic>> {
        let design = Elab::design(src);
        assert!(design.diagnostics().is_empty(), "the source must be clean: {:?}", design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>());
        Elaborator::new(&design).run(top)
    }

    /// Elaborate `top` of `src` and expect an `Ir` that passes `check()`.
    pub fn ir(src: &str, top: &str) -> Ir {
        let ir = Elab::run(src, top).unwrap_or_else(|d| panic!("elaboration refused: {d:?}"));
        assert!(ir.check().is_empty(), "{:?}", ir.check());
        ir
    }

    pub fn codes(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
        diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
    }
}
