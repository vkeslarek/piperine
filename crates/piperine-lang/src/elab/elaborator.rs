//! The entry point and the pass order of one elaboration run: `Types::check` first, then the
//! passes over what it reached, each writing through `IrBuilder`.

use std::collections::BTreeMap;

use piperine_ir::{BodyId, Ir, IrBuilder, NatureId};

use super::body::ModulePlan;
use super::consts::ConstEval;
use super::origin::Locator;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, Design, DesignDiagnostic, FileId};
use crate::types::Types;

/// The entry point: `Elaborator::new(&design).run("Top")`.
#[derive(Debug, Clone, Copy)]
pub struct Elaborator<'d> {
    design: &'d Design,
}

impl<'d> Elaborator<'d> {
    pub fn new(design: &'d Design) -> Self {
        Elaborator { design }
    }

    /// Elaborate `top`, a module of the root crate without a port.
    ///
    /// The front-end and type-check diagnostics come back unchanged; an unknown top is one
    /// diagnostic naming it, and so is a top with a port (a port of a top has nothing to connect
    /// to, and caller-given values are M2).
    pub fn run(self, top: &str) -> Result<Ir, Vec<DesignDiagnostic>> {
        let types = Types::check(self.design, top)?;
        let top_decl = types.reached()[0];
        Elaboration::new(self.design, &types).run(top_decl)
    }
}

/// The state of one run. Its passes are `impl` blocks in the files named after them.
pub(super) struct Elaboration<'d, 't> {
    pub(super) design: &'d Design,
    pub(super) types: &'t Types<'d>,
    pub(super) consts: ConstEval<'d, 't>,
    pub(super) ir: IrBuilder,
    pub(super) locator: Locator<'d>,
    pub(super) natures: BTreeMap<DeclId, NatureId>,
    /// The body each reached module will get: bodies are built in first-reach order, so a
    /// placement can name its child's body before that body exists.
    pub(super) body_ids: BTreeMap<DeclId, BodyId>,
    pub(super) plans: BTreeMap<DeclId, ModulePlan>,
    pub(super) diagnostics: Vec<DesignDiagnostic>,
}

impl<'d, 't> Elaboration<'d, 't> {
    fn new(design: &'d Design, types: &'t Types<'d>) -> Self {
        Elaboration {
            design,
            types,
            consts: ConstEval::new(design, types),
            ir: IrBuilder::new(),
            locator: Locator::new(design),
            natures: BTreeMap::new(),
            body_ids: BTreeMap::new(),
            plans: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self, top: DeclId) -> Result<Ir, Vec<DesignDiagnostic>> {
        if self.top_has_a_port(top) {
            return Err(self.diagnostics);
        }
        let modules: Vec<DeclId> = self.types.reached().iter().copied().filter(|d| matches!(self.design.decl_data(*d).kind, DeclKind::Module(_))).collect();
        self.body_ids = modules.iter().enumerate().map(|(k, m)| (*m, BodyId(k as u32))).collect();
        self.lower_natures(&modules);
        for module in &modules {
            self.lower_body(*module);
        }
        let top_instance = self.place_top(top);
        match top_instance {
            Some(instance) if self.diagnostics.is_empty() => {
                let ir = self.ir.finish(instance);
                debug_assert!(ir.check().is_empty(), "elaboration produced an ill-formed IR: {:?}", ir.check());
                Ok(ir)
            }
            _ => Err(self.diagnostics),
        }
    }

    /// `P0001` and `true` when the top has a port.
    fn top_has_a_port(&mut self, top: DeclId) -> bool {
        let data = self.design.decl_data(top);
        let has_port = data.children.iter().any(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_)));
        if has_port {
            let name = data.name.as_ref().map_or("", |n| n.text.as_str());
            let (file, span) = (data.file, data.name.as_ref().map_or(data.span, |n| n.span));
            self.report(file, Diagnostic::new("P0001", format!("top module `{name}` has a port, which is not supported yet"), span, "a top module has no port in M1"));
        }
        has_port
    }

    pub(super) fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        self.diagnostics.push(DesignDiagnostic { file, diagnostic });
    }

    /// `P0001`: a construct inside the type-checked subset that M1 elaboration does not lower.
    pub(super) fn unsupported(&mut self, file: FileId, span: Span, what: &str) {
        self.report(file, Diagnostic::new("P0001", format!("{what} is not supported yet"), span, format!("{what} is outside the M1 subset")));
    }
}
