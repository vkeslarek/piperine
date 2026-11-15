//! The entry point and the pass order of one elaboration run: `Types::check` first, then the
//! passes over what it reached, each writing through `IrBuilder`.

use piperine_ir::{BodySpec, InstanceSpec, Ir, IrBuilder};

use crate::msl::Diagnostic;
use crate::pom::{DeclId, DeclKind, Design, DesignDiagnostic, FileId};
use crate::types::Types;

use super::origin::Locator;

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
        Elaboration::new(self.design).run(top_decl)
    }
}

/// The state of one run.
struct Elaboration<'d> {
    design: &'d Design,
    ir: IrBuilder,
    locator: Locator<'d>,
    diagnostics: Vec<DesignDiagnostic>,
}

impl<'d> Elaboration<'d> {
    fn new(design: &'d Design) -> Self {
        Elaboration { design, ir: IrBuilder::new(), locator: Locator::new(design), diagnostics: Vec::new() }
    }

    fn run(mut self, top: DeclId) -> Result<Ir, Vec<DesignDiagnostic>> {
        if self.top_has_a_port(top) {
            return Err(self.diagnostics);
        }
        let data = self.design.decl_data(top);
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let origin = self.locator.decl(top);
        let body = self.ir.body(BodySpec {
            name: name.clone(),
            origin: origin.clone(),
            params: Vec::new(),
            nodes: Vec::new(),
            branches: Vec::new(),
            limits: Vec::new(),
            stims: Vec::new(),
            locals: Vec::new(),
            analog: None,
            children: Vec::new(),
        });
        let instance = self.ir.instance(InstanceSpec { path: name.clone(), name, body, parent: None, nodes: Vec::new(), params: Vec::new(), origin });
        if !self.diagnostics.is_empty() {
            return Err(self.diagnostics);
        }
        let ir = self.ir.finish(instance);
        debug_assert!(ir.check().is_empty(), "elaboration produced an ill-formed IR: {:?}", ir.check());
        Ok(ir)
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

    fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        self.diagnostics.push(DesignDiagnostic { file, diagnostic });
    }
}
