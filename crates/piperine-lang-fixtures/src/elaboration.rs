//! The elaboration tests' design builder: a design over the real standard library, elaborated, and its diagnostic codes.

use piperine_ir::Ir;
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::{Design, DesignDiagnostic};

/// A source elaborated over the real standard library.
pub struct SourceElaboration;

impl SourceElaboration {
    /// A design of one in-memory root file `top.msl` over the real standard library.
    pub fn design(src: &str) -> Design {
        Design::builder().source("top.msl", src).build()
    }

    /// Elaborate `top` of `src`; the design must hold no diagnostic of its own.
    pub fn run(src: &str, top: &str) -> Result<Ir, Vec<DesignDiagnostic>> {
        let design = SourceElaboration::design(src);
        assert!(design.diagnostics().is_empty(), "the source must be clean: {:?}", design.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect::<Vec<_>>());
        Elaborator::new(&design).run(top)
    }

    /// Elaborate `top` of `src` and expect an `Ir` that passes `check()`.
    pub fn ir(src: &str, top: &str) -> Ir {
        let ir = SourceElaboration::run(src, top).unwrap_or_else(|d| panic!("elaboration refused: {d:?}"));
        assert!(ir.check().is_empty(), "{:?}", ir.check());
        ir
    }

    /// The path of the node each port and signal of the instance at `path` is bound to, in the instance's node order.
    pub fn node_paths(ir: &Ir, path: &str) -> Vec<String> {
        let instance = ir.instance_at(path).unwrap_or_else(|| panic!("no instance {path}"));
        instance
            .bindings()
            .iter()
            .map(|bound| match bound {
                piperine_ir::NodeRef::Node(id) => ir.node(*id).path().to_string(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    /// The code of every diagnostic, in order.
    pub fn codes(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
        diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
    }
}
